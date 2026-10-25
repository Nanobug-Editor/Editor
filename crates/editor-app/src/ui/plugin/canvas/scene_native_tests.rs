//! Real D3D readback validates scene pixels through the ordinary native plugin projection.
use super::*;
use std::{
    cell::RefCell,
    sync::Arc,
    time::{Duration, Instant},
};
mod dpi;
mod wasm;

/// One isolated OS window proves that a transformed public scene reaches native pixels.
#[test]
#[ignore = "opens a bounded Windows/D3D test window; run explicitly with --test-threads=1"]
fn native_canvas_scene_paints_transformed_shapes() {
    const CHILD: &str = "NANOBUG_CANVAS_SCENE_NATIVE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let mut log = tempfile::tempfile().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "native_canvas_scene_paints_transformed_shapes",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env("RUST_MIN_STACK", "8388608")
            .stdout(log.try_clone().unwrap())
            .stderr(log.try_clone().unwrap())
            .spawn()
            .unwrap();
        // A real package may consume its bounded 60 s cold demand before the native 30 s portion.
        let deadline = Instant::now() + Duration::from_secs(90);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() > deadline {
                child.kill().unwrap();
                panic!("native canvas child timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        use std::io::{Read as _, Seek as _};
        log.rewind().unwrap();
        let mut output = String::new();
        log.read_to_string(&mut output).unwrap();
        eprintln!("{output}");
        assert!(status.success(), "native canvas child failed: {status}");
        return;
    }
    let case = std::env::var("NANOBUG_CANVAS_SCENE_CASE").unwrap_or_else(|_| "basic".into());
    assert!(
        matches!(case.as_str(), "basic" | "layers" | "image" | "custom"),
        "unknown scene acceptance case"
    );
    let fixture = if case == "custom" {
        let path = std::env::var_os("NANOBUG_CANVAS_SCENE_INPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/ui/plugin/canvas/scene_test_data/presentation.json")
            });
        assert!(
            std::fs::metadata(&path).unwrap().len() <= 2 * 1024 * 1024,
            "native fixture exceeds its budget"
        );
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
    } else {
        serde_json::Value::Null
    };
    let installed = fixture["wasm_flow"]
        .as_bool()
        .unwrap_or(false)
        .then(wasm::install);
    let managed_document = installed.as_ref().map(|owner| owner.document.clone());
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    gpui_kit::application().with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit).run(move |cx| {
            gpui_kit::init(cx);editor_ui::typography::init(cx);
            editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(fixture["dark"].as_bool().unwrap_or(false)),cx);
            let mut wire=serde_json::to_value(Canvas::default()).unwrap();
            wire["scene"]=serde_json::json!({"identity":1,"revision":1,"roots":[
                {"kind":"group","id":"moved","transform":[1,0,0,1,20,30],"children":[
                    {"kind":"shape","id":"box","geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":100,"h":80}},
                     "fill":{"kind":"solid","rgba":4097793791u32}}]}]});
            if case=="layers" {
                wire["scene"]=serde_json::json!({"identity":2,"revision":1,"roots":[
                    {"kind":"group","id":"isolated","opacity":0.5,
                        "clip":{"kind":"rect","rect":{"x":20,"y":20,"w":80,"h":80}},
                        "children":[
                            {"kind":"shape","id":"left","geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":80,"h":80}},"fill":{"kind":"solid","rgba":4278190335u32}},
                            {"kind":"shape","id":"right","geometry":{"kind":"rect","rect":{"x":40,"y":0,"w":80,"h":80}},"fill":{"kind":"solid","rgba":4278190335u32}}]}]});
            }
            if case=="image" {
                wire["scene"]=serde_json::json!({"identity":3,"revision":1,"roots":[
                    {"kind":"image","id":"photo","image":"pixels","rect":{"x":20,"y":20,"w":100,"h":80}}]});
            }
            if case=="custom"&&!fixture["scene"].is_null() {wire["scene"]=fixture["scene"].clone();}
            let canvas:Canvas=serde_json::from_value(wire).unwrap();
            let mut document=Document::new(ui::Node::new("drawing",Kind::Canvas(canvas)).grow()).revision(1);
            if let Some(managed)=managed_document {document=managed;}
            if let Some(copies)=fixture["copies"].as_u64() {
                assert!((1..=8).contains(&copies),"native scene fixture copies exceed their budget");
                let children=(0..copies).map(|index| {
                    let mut child=document.root.clone();
                    child.id=if index==0 {"drawing".into()}else{format!("drawing{index}")};
                    child.layout.height=Some(40.);
                    if let Kind::Canvas(canvas)=&mut child.kind {canvas.scene.as_mut().unwrap().identity+=index;}
                    child
                }).collect();
                document.root=ui::Node::column("scenes",children).grow();
            }
            let mut resources=BTreeMap::new();
            if case=="image" {
                let version=protocol::api::DocumentVersion{id:"scene-source".into(),path:"scene.fixture".into(),revision:1};
                let mut declared=ui::Node::new("pixels",Kind::Image{source:"local.png".into(),alt:"native blue reference".into()});
                declared.layout.width=Some(0.);declared.layout.height=Some(0.);
                document.root=ui::Node::column("root",vec![declared,document.root]).grow();document.source=Some(version.clone());
                let pixels=image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4,4,image::Rgba([0,153,255,255])));
                let mut encoded=std::io::Cursor::new(Vec::new());pixels.write_to(&mut encoded,image::ImageFormat::Png).unwrap();
                resources.insert("ordinary/diagram/image/pixels".into(),Arc::new(plugin_runtime::ImageResource{
                    source:version.into(),uri:"local.png".into(),state:plugin_runtime::ImageState::Ready(Arc::new(encoded.into_inner()))}));
            }
            document.validate().unwrap();
            let expected_scene=document.active_node("drawing").and_then(|node|if let Kind::Canvas(canvas)=&node.kind {canvas.scene.as_ref()}else{None}).map(|scene|(scene.identity,scene.revision));
            let events=Rc::new(RefCell::new(Vec::new()));let record=events.clone();
            let mut renderer=super::super::images::VectorRenderer::default();
            let images=renderer.prepare_resources(&BTreeMap::from([("ordinary/diagram".into(),Arc::new(document.clone()))]),&resources);
            let retained=Rc::new(RefCell::new(None));let saved=retained.clone();
            let window=cx.open_window(gpui_kit::WindowOptions {
                window_bounds:Some(gpui_kit::WindowBounds::Windowed(Bounds::new(point(px(100.),px(100.)),size(px(320.),px(240.))))),
                ..Default::default()
            },move |window,cx| {
                let view=cx.new(|cx|PluginView::new("ordinary".into(),document,Environment::default(),move |event,_|record.borrow_mut().push(event),window,cx));
                *saved.borrow_mut()=Some(view.clone());
                view.update(cx,|view,cx|view.update_images("ordinary/diagram",&images,window,cx));
                cx.new(|cx|gpui_base::Root::new(view,window,cx))
            }).unwrap();
            cx.spawn(async move |cx| {
                // Retain the real incarnation and its isolated workspace until native retirement.
                let _owner=installed;
                cx.background_executor().timer(Duration::from_millis(800)).await;
                let requested_dpi=fixture["dpi_percent"].as_u64().map(|value|value as u32);
                if let Some(percent)=requested_dpi {
                    let native=window.update(cx,|_,window,_|dpi::NativeDpi::capture(window)).and_then(|value|value);
                    if let Ok(native)=native {
                        let _=cx.background_executor().spawn(async move {native.apply(percent)}).await;
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                    }
                }
                let mut observed=window.update(cx,|_,window,cx|->anyhow::Result<()> {
                    let image=window.render_to_image()?;let scale=window.scale_factor();
                    image.save(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(format!("canvas-scene-{case}.png")))?;
                    if case=="custom" {
                        if let Some(percent)=requested_dpi {anyhow::ensure!((scale-percent as f32/100.).abs()<0.01,"native DPI change did not apply: requested {percent}, observed scale {scale}");}
                        if let Some(samples)=fixture["samples"].as_array() {for sample in samples {
                            let (x,y)=(sample["x"].as_f64().unwrap() as f32,sample["y"].as_f64().unwrap() as f32);
                            let actual=image.get_pixel((x*scale) as u32,(y*scale) as u32).0;
                            let expected=sample["rgba"].as_array().unwrap();let tolerance=sample["tolerance"].as_u64().unwrap_or(3) as i32;
                            anyhow::ensure!(expected.len()==4&&actual.into_iter().zip(expected).all(|(a,e)|(i32::from(a)-e.as_i64().unwrap() as i32).abs()<=tolerance),"scene pixel at ({x},{y}): expected {expected:?}, observed {actual:?}");
                        }}
                        if let Some(samples)=fixture["canvas_samples"].as_array() {
                            let retained=retained.borrow();
                            let view=retained.as_ref().unwrap().read(cx);
                            let snapshot=view.native_canvas_input_snapshot("drawing",window,cx).expect("native canvas geometry");
                            for sample in samples {
                                let x=snapshot["origin_x"].as_f64().unwrap()+sample["x"].as_f64().unwrap();
                                let y=snapshot["origin_y"].as_f64().unwrap()+sample["y"].as_f64().unwrap();
                                let actual=image.get_pixel((x as f32*scale) as u32,(y as f32*scale) as u32).0;
                                let expected=sample["rgba"].as_array().unwrap();
                                anyhow::ensure!(expected.len()==4&&actual.into_iter().zip(expected).all(|(a,e)|(i64::from(a)-e.as_i64().unwrap()).abs()<=4),"projected canvas pixel at ({x},{y}): expected {expected:?}, observed {actual:?}");
                            }
                        }
                        if let Some(colors)=fixture["visible_colors"].as_array() {for color in colors {
                            let expected=color["rgba"].as_array().unwrap();
                            let count=image.pixels().filter(|pixel|pixel.0.into_iter().zip(expected).all(|(a,e)|(i64::from(a)-e.as_i64().unwrap()).abs()<=4)).count();
                            anyhow::ensure!(count>=color["minimum"].as_u64().unwrap() as usize,"actual WASM scene color {expected:?} missing: {count} pixels");
                        }}
                        if fixture["expect_presented"].as_bool().unwrap_or(false) {
                            let receipts=events.borrow().iter().filter_map(|event|match &event.action {
                                ui::Action::Canvas(CanvasEvent::ScenePresented{identity,scene_revision,error,..})
                                |ui::Action::ViewportInput(ui::ViewportInput{event:CanvasEvent::ScenePresented{identity,scene_revision,error,..},..})=>Some((*identity,*scene_revision,error.clone())),_=>None
                            }).collect::<Vec<_>>();
                            let expected=expected_scene.expect("receipt acceptance requires a published scene");
                            anyhow::ensure!(receipts==vec![(expected.0,expected.1,None)],"expected one exact native presentation receipt, observed {receipts:?}");
                        }
                        if let Some(failed)=fixture["expect_failed_scene"].as_u64() {
                            let receipts=events.borrow().iter().filter_map(|event|match &event.action {
                                ui::Action::Canvas(CanvasEvent::ScenePresented{identity,error,..})=>Some((*identity,error.is_some())),_=>None
                            }).collect::<Vec<_>>();
                            anyhow::ensure!(receipts.len()==fixture["copies"].as_u64().unwrap() as usize
                                &&receipts.iter().filter(|(_,error)|*error).map(|(id,_)|*id).collect::<Vec<_>>()==vec![failed],
                                "derived pixel quota must reject only the final canvas: {receipts:?}");
                        }
                        return Ok(());
                    }
                    if case=="layers" {
                        for x in [40.,70.] {
                            let pixel=image.get_pixel((x*scale) as u32,(50.*scale) as u32).0;
                            anyhow::ensure!(pixel[0]>250&&(125..=131).contains(&pixel[1])&&(125..=131).contains(&pixel[2]),"isolated layer must be uniformly pink, observed {pixel:?} at {x}");
                        }
                        let outside=image.get_pixel((110.*scale) as u32,(50.*scale) as u32).0;
                        anyhow::ensure!(outside[..3]==[255,255,255],"clip must retain the white background, observed {outside:?}");
                        return Ok(());
                    }
                    if case=="image" {
                        for x in [21.,60.,119.] {
                            let pixel=image.get_pixel((x*scale) as u32,(60.*scale) as u32).0;
                            anyhow::ensure!(pixel[0]<5&&(150..=156).contains(&pixel[1])&&pixel[2]>250,"scene image must retain blue pixels through its scaled edges, observed {pixel:?} at {x}");
                        }
                        return Ok(());
                    }
                    let pixel=image.get_pixel((60.*scale) as u32,(60.*scale) as u32).0;
                    anyhow::ensure!(pixel[0]>200&&pixel[1]<120&&pixel[2]<160,"expected transformed red rectangle, observed {pixel:?}");
                    Ok(())
                }).and_then(|value|value);
                // Close the owned HWND while AccessKit/IME thread state is alive, then drain events.
                retained.borrow_mut().take();
                if let Err(error)=window.update(cx,|_,window,_|window.remove_window()) {
                    if observed.is_ok() {observed=Err(error);}
                }
                cx.background_executor().timer(Duration::from_millis(150)).await;
                if !cx.update(|cx|cx.windows().is_empty())&&observed.is_ok() {
                    observed=Err(anyhow::anyhow!("native canvas window did not retire before application shutdown"));
                }
                *output.borrow_mut()=Some(observed);
                let _=cx.update(|cx|cx.quit());
            }).detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native canvas readback must run")
        .unwrap();
}
