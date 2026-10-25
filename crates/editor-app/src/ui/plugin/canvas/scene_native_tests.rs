//! Real D3D readback validates scene pixels through the ordinary native plugin projection.
use super::*;
use std::{
    cell::RefCell,
    sync::Arc,
    time::{Duration, Instant},
};
mod dpi;
mod ime;
mod input;
mod layout;
mod resources;
mod uia;
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
    let installed = fixture["wasm_editable"]
        .as_str()
        .map(wasm::install_editable)
        .or_else(|| {
            fixture["wasm_flow"]
                .as_bool()
                .unwrap_or(false)
                .then(wasm::install)
        });
    let managed_document = installed.as_ref().map(|owner| owner.document.clone());
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    let platform = gpui_kit::platform::current_platform(false);
    let fonts = platform.text_system();
    gpui_kit::Application::with_platform(platform).with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit).run(move |cx| {
            editor_ui::text::paragraph::install_native_raster_fonts(fonts,cx).unwrap();
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
            if !fixture["text_queries"].is_null() {wire["text_queries"]=fixture["text_queries"].clone();}
            if let Some(count)=fixture["image_repetitions"].as_u64() {
                assert!((1..=32).contains(&count));
                let original=wire["scene"]["roots"][0]["children"][0].clone();
                wire["scene"]["roots"][0]["children"]=serde_json::Value::Array((0..count).map(|index|{
                    let mut image=original.clone();image["id"]=format!("image{index}").into();image
                }).collect());
            }
            let canvas:Canvas=serde_json::from_value(wire).unwrap();
            let mut document=Document::new(ui::Node::new("drawing",Kind::Canvas(canvas)).grow()).revision(1);
            if let Some(managed)=managed_document {document=managed;}
            if fixture["legacy_rtl_reference"].as_bool().unwrap_or(false) {
                document.root.layout.height=Some(100.);
                let mut reference=ui::Node::new("reference",Kind::Canvas(Canvas{paint:vec![protocol::Paint::Text{
                    x:20.,y:20.,text:"גבא".into(),color:0,size:18.,bold:false,font:Some("Segoe UI".into())
                }],..Default::default()}));
                reference.layout.height=Some(100.);
                document.root=ui::Node::column("root",vec![document.root,reference]).grow();
            }
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
            resources::attach(&fixture,&mut document,&mut resources);
            document.validate().unwrap();
            let expected_scene=document.active_node("drawing").and_then(|node|if let Kind::Canvas(canvas)=&node.kind {canvas.scene.as_ref()}else{None}).map(|scene|(scene.identity,scene.revision));
            let events=Rc::new(RefCell::new(Vec::new()));let record=events.clone();
            let mut renderer=super::super::images::VectorRenderer::default();
            let documents=BTreeMap::from([("ordinary/diagram".into(),Arc::new(document.clone()))]);
            let deferred=fixture["prepare_worker"].as_bool().unwrap_or(false);
            let images=if deferred {super::super::images::SceneImages::default()}else{renderer.prepare_resources(&documents,&resources)};
            let preparation=deferred.then_some((renderer,documents,resources));
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
            // Native dispatch may draw Root; do not retain a typed Root update while dispatching.
            let window:gpui_kit::AnyWindowHandle=window.into();
            cx.spawn(async move |cx| {
                // Retain the real incarnation and its isolated workspace until native retirement.
                let mut owner=installed;
                if let Some((mut renderer,documents,resources))=preparation {
                    let images=cx.background_executor().spawn(async move {renderer.prepare_resources(&documents,&resources)}).await;
                    window.update(cx,|_,window,cx| {
                        retained.borrow().as_ref().unwrap().update(cx,|view,cx|view.update_images("ordinary/diagram",&images,window,cx));
                        window.draw(cx).clear(cx);
                    }).unwrap();
                }
                cx.background_executor().timer(Duration::from_millis(800)).await;
                let requested_dpi=fixture["dpi_percent"].as_u64().map(|value|value as u32);
                if let Some(percent)=requested_dpi {
                    let native=window.update(cx,|_,window,_|dpi::NativeDpi::capture(window)).and_then(|value|value);
                    if let Ok(native)=native {
                        let _=cx.background_executor().spawn(async move {native.apply(percent)}).await;
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                    }
                }
                if fixture["target_key_script"].as_bool().unwrap_or(false) {
                    window.update(cx,|_,window,cx| {
                        let snapshot=retained.borrow().as_ref().unwrap().read(cx).native_canvas_input_snapshot("drawing",window,cx).unwrap();
                        input::keys(&snapshot,window,cx)
                    }).unwrap().unwrap();
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                }
                if fixture["target_button_chord_script"].as_bool().unwrap_or(false) {
                    window.update(cx,|_,window,cx| {
                        let snapshot=retained.borrow().as_ref().unwrap().read(cx).native_canvas_input_snapshot("drawing",window,cx).unwrap();
                        input::chords::run(&snapshot,window,cx)
                    }).unwrap();
                }
                if fixture["target_pointer_script"].as_bool().unwrap_or(false) {
                    let mut activation_peer=None;
                    for phase in 0..3 {
                        if phase==1&&fixture["target_pointer_cancel"]=="window" {
                            activation_peer=Some(cx.update(input::deactivation::open).unwrap());
                            cx.background_executor().timer(Duration::from_millis(150)).await;
                            window.update(cx,|_,window,_|{anyhow::ensure!(!window.is_window_active(),"owned activation peer must deactivate the Canvas window");Ok(())}).unwrap().unwrap();
                            continue;
                        }
                        if phase==2 {
                            if let Some(peer)=activation_peer.take() {peer.update(cx,|_,window,_|window.remove_window()).unwrap();}
                        }
                        window.update(cx,|_,window,cx| {
                            if phase==1 {
                                if let Some(cancel)=fixture["target_pointer_cancel"].as_str() {
                                    super::targets::native_text_tests::redraw(retained.borrow().as_ref().unwrap(),|document| {
                                        if cancel=="modal" {document.dialog=Some(ui::Dialog::new("modal","Modal / 模态",ui::Node::button("close","关闭 / Close")));}
                                        else if let Kind::Canvas(canvas)=&mut document.root.kind {canvas.scene.as_mut().unwrap().targets[0].epoch=2;}
                                    },window,cx)?;
                                    return Ok(());
                                }
                            }
                            let snapshot=retained.borrow().as_ref().unwrap().read(cx).native_canvas_input_snapshot("drawing",window,cx).unwrap();
                            input::pointer(&snapshot,phase,window,cx)
                        }).unwrap().unwrap();
                        cx.background_executor().timer(Duration::from_millis(50)).await;
                    }
                }
                if fixture["target_ime_script"].as_bool().unwrap_or(false) {
                    let mut evidence=super::targets::native_text_tests::Evidence::default();
                    for phase in 0..9 {
                        window.update(cx,|_,window,cx|super::targets::native_text_tests::phase(
                            retained.borrow().as_ref().unwrap(),&events.borrow(),&mut evidence,phase,window,cx)).unwrap().unwrap();
                        cx.background_executor().timer(Duration::from_millis(60)).await;
                    }
                }
                let semantic_result=if fixture["semantic_probe"].as_bool().unwrap_or(false) {
                    let handle=window.update(cx,|_,window,_|uia::owned_handle(window)).unwrap().unwrap();
                    Some(cx.background_executor().spawn(async move {uia::probe(handle)}).await)
                }else{None};
                let mut interaction_error=None;
                if fixture["rounded_hit_script"].as_bool().unwrap_or(false) {
                    if let Err(error)=window.update(cx,|_,window,cx|input::hits::run(retained.borrow().as_ref().unwrap(),window,cx)).and_then(|result|result) {interaction_error=Some(error);}
                    if interaction_error.is_none() {if let Err(error)=input::hits::verify(&events.borrow()) {interaction_error=Some(error);}}
                }
                if fixture["pending_viewport_wheel"].as_bool().unwrap_or(false) {
                    if let Err(error)=window.update(cx,|_,window,cx|input::wheel::run(retained.borrow().as_ref().unwrap(),window,cx)).and_then(|result|result) {interaction_error=Some(error);}
                    if interaction_error.is_none() {if let Err(error)=input::wheel::verify(&events.borrow()) {interaction_error=Some(error);}}
                }
                if fixture["native_layout_stress"].as_bool().unwrap_or(false) {
                    if let Err(error)=window.update(cx,|_,window,_|layout::stress(window)).and_then(|result|result) {interaction_error=Some(error);}
                }
                if let Some(kind)=fixture["wasm_editable"].as_str() {
                    let view=retained.borrow().as_ref().unwrap().clone();
                    if let Err(error)=wasm::editable::run(kind,owner.as_mut().unwrap(),&view,&events,window,cx).await {interaction_error=Some(error);}
                }
                if fixture["target_affine_ime_script"].as_bool().unwrap_or(false) {
                    let mut evidence=input::draft::Evidence::default();
                    for phase in 0..3 {
                        if let Err(error)=window.update(cx,|_,window,cx|input::draft::phase(retained.borrow().as_ref().unwrap(),&mut evidence,phase,window,cx)).and_then(|result|result) {interaction_error=Some(error);break;}
                        cx.background_executor().timer(Duration::from_millis(500)).await;
                    }
                }
                if fixture["target_edit_keys_script"].as_bool().unwrap_or(false) {
                    if let Err(error)=window.update(cx,|_,window,cx|input::editing::run(retained.borrow().as_ref().unwrap(),window,cx)).and_then(|result|result) {interaction_error=Some(error);}
                }
                if fixture["target_os_ime_script"].as_bool().unwrap_or(false) {
                    let mut evidence=ime::Evidence::default();
                    for phase in 0..6 {
                        if let Err(error)=window.update(cx,|_,window,cx|ime::phase(retained.borrow().as_ref().unwrap(),&mut evidence,phase,window,cx)).and_then(|result|result) {
                            interaction_error=Some(error);break;
                        }
                        cx.background_executor().timer(Duration::from_millis(450)).await;
                    }
                }
                if let Some(target)=fixture["semantic_editable_target"].as_str() {
                    let handle=window.update(cx,|_,window,_|uia::owned_handle(window)).unwrap().unwrap();
                    let target=target.to_owned();
                    match cx.background_executor().spawn(async move {wasm::editable::uia::probe(handle,&target,false)}).await {
                        Ok(result)=> {eprintln!("OS IME semantic focus isolation: {result}");if result["focus"]!=true {interaction_error=Some(anyhow::anyhow!("OS IME native semantic focus: {result}"));}},
                        Err(error)=>interaction_error=Some(error),
                    }
                }
                let mut observed=window.update(cx,|_,window,cx|->anyhow::Result<()> {
                    if let Some(error)=&interaction_error {anyhow::bail!("OS interaction: {error:#}");}
                    let image=window.render_to_image()?;let scale=window.scale_factor();
                    image.save(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(format!("canvas-scene-{case}.png")))?;
                    if case=="custom" {
                        if let Some(result)=&semantic_result {uia::verify(result.as_ref().map_err(|error|anyhow::anyhow!("{error:#}"))?,&events.borrow(),scale)?;}
                        if fixture["target_os_ime_script"].as_bool().unwrap_or(false) {ime::verify(&events.borrow())?;}
                        if fixture["target_key_script"].as_bool().unwrap_or(false) {input::verify_keys(&events.borrow())?;}
                        if fixture["target_button_chord_script"].as_bool().unwrap_or(false) {input::chords::verify(&events.borrow())?;}
                        if fixture["target_edit_keys_script"].as_bool().unwrap_or(false) {input::editing::verify(&events.borrow())?;}
                        if fixture["target_pointer_script"].as_bool().unwrap_or(false) {
                            let reason=match fixture["target_pointer_cancel"].as_str() {Some("epoch")=>protocol::ui::CanvasCancelReason::EpochReplaced,
                                Some("modal")=>protocol::ui::CanvasCancelReason::Modal,_=>protocol::ui::CanvasCancelReason::FocusLost};
                            input::verify_pointer(&events.borrow(),reason)?;
                        }
                        if fixture["target_ime_script"].as_bool().unwrap_or(false) {super::targets::native_text_tests::verify(&events.borrow())?;}
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
                            anyhow::ensure!(count>=color["minimum"].as_u64().unwrap() as usize,"native scene color {expected:?} missing: {count} pixels");
                        }}
                        if let Some(minimum)=fixture["native_color_glyph_minimum"].as_u64() {
                            let count=image.pixels().filter(|pixel|pixel.0[..3].iter().max().unwrap()-pixel.0[..3].iter().min().unwrap()>32).count();
                            anyhow::ensure!(count>=minimum as usize,"worker-prepared native color emoji became monochrome: {count} pixels");
                        }
                        if fixture["expect_presented"].as_bool().unwrap_or(false)&&!fixture["target_ime_script"].as_bool().unwrap_or(false)&&!fixture["target_os_ime_script"].as_bool().unwrap_or(false) {
                            let receipts=events.borrow().iter().filter_map(|event|match &event.action {
                                ui::Action::Canvas(CanvasEvent::ScenePresented{identity,scene_revision,error,..})
                                |ui::Action::ViewportInput(ui::ViewportInput{event:CanvasEvent::ScenePresented{identity,scene_revision,error,..},..})=>Some((*identity,*scene_revision,error.clone())),_=>None
                            }).collect::<Vec<_>>();
                            let expected=expected_scene.expect("receipt acceptance requires a published scene");
                            anyhow::ensure!(receipts==vec![(expected.0,expected.1,None)],"expected one exact native presentation receipt, observed {receipts:?}");
                        }
                        if fixture["expected_text_hits"].is_array()||fixture["allowed_text_boundaries"].is_array() {
                            let reports=events.borrow().iter().filter_map(|event| {
                                let wire=serde_json::to_value(&event.action).unwrap();
                                (wire["type"]=="canvas"&&wire["value"]["type"]=="text_layout").then(||wire["value"].clone())
                            }).collect::<Vec<_>>();
                            anyhow::ensure!(reports.len()==1,"expected one addressed native text layout result, observed {reports:?}");
                            let report=&reports[0];
                            let scene=expected_scene.expect("text queries require a scene");
                            anyhow::ensure!(report["identity"]==scene.0&&report["scene_revision"]==scene.1
                                &&report["results"][0]["font_environment"].as_u64().is_some_and(|key|key!=0),
                                "native text positions must retain original UTF-8 offsets and actual font binding: {report:?}");
                            if let Some(expected)=fixture["expected_text_hits"].as_array() {
                                anyhow::ensure!(report["results"][0]["hits"]==serde_json::Value::Array(expected.clone()),"native original text offsets differ: {report:?}");
                            }
                            if let Some(allowed)=fixture["allowed_text_boundaries"].as_array() {
                                let hits=report["results"][0]["hits"].as_array().expect("native hit list");
                                anyhow::ensure!(hits.len()==fixture["text_queries"][0]["points"].as_array().unwrap().len()
                                    &&hits.iter().all(|hit|allowed.contains(hit))&&hits.iter().any(|hit|hit.as_u64().unwrap()>=23),
                                    "native carets must stay on whole graphemes across combining marks, emoji, RTL and CRLF: {hits:?}");
                            }
                            if fixture["expected_affine_text_geometry"].as_bool().unwrap_or(false) {
                                let rect=&report["results"][0]["positions"][0]["rect"];
                                for (field,expected) in [("x",171.),("y",20.),("w",29.),("h",1.)] {
                                    anyhow::ensure!((rect[field].as_f64().unwrap()-expected).abs()<0.01,"affine native caret {field} differs: {rect}");
                                }
                                anyhow::ensure!(report["results"][0]["positions"][2]["rect"].is_null(),"a caret beyond the rotated group clip must remain hidden: {report}");
                            }
                            if fixture["expected_text_geometry"].as_bool().unwrap_or(false) {
                                let result=&report["results"][0];
                                let carets=result["positions"].as_array().expect("native caret rectangles");
                                anyhow::ensure!(carets.len()==2&&carets[0]["byte"]==0&&carets[1]["byte"]==17,
                                    "caret rectangles must address original bytes: {result:?}");
                                for (caret,y) in carets.iter().zip([20.,44.]) {
                                    let rect=&caret["rect"];
                                    anyhow::ensure!((rect["x"].as_f64().unwrap()-20.).abs()<0.01
                                        &&(rect["y"].as_f64().unwrap()-y).abs()<0.01
                                        &&(rect["h"].as_f64().unwrap()-24.).abs()<0.01,"native line geometry differs: {caret:?}");
                                }
                                let rects=result["selections"][0]["rects"].as_array().expect("native selection fragments");
                                anyhow::ensure!(rects.len()==2&&rects[0]["y"]==20.
                                    &&rects[0]["w"].as_f64().unwrap()>1.
                                    &&rects[1]["y"]==44.&&rects[1]["x"]==20.
                                    &&(rects[1]["w"].as_f64().unwrap()-36.).abs()<2.
                                    &&rects[1]["h"]==24.,"native multiline selection must cover the final Latin glyph and two full-width Chinese glyphs: {rects:?}");
                            }
                            if fixture["expected_rtl_carets"].as_bool().unwrap_or(false) {
                                let carets=report["results"][0]["positions"].as_array().expect("native bidi carets");
                                anyhow::ensure!(carets.len()==4&&carets.windows(2).all(|pair|
                                    pair[0]["rect"]["x"].as_f64().unwrap()>pair[1]["rect"]["x"].as_f64().unwrap()),
                                    "logical Hebrew positions must move right to left: {carets:?}");
                                let retained=retained.borrow();let view=retained.as_ref().unwrap().read(cx);
                                let top=view.native_canvas_input_snapshot("drawing",window,cx).unwrap();
                                let reference=view.native_canvas_input_snapshot("reference",window,cx).unwrap();
                                let mut ink=0usize;let mut changed=0usize;
                                for dy in 0..(50.*scale) as u32 {for dx in 0..(100.*scale) as u32 {
                                    let dark=|snapshot:&serde_json::Value| {
                                        let x=((snapshot["origin_x"].as_f64().unwrap()+20.)*f64::from(scale)) as u32+dx;
                                        let y=((snapshot["origin_y"].as_f64().unwrap()+20.)*f64::from(scale)) as u32+dy;
                                        let pixel=image.get_pixel(x,y).0;pixel[0]<100&&pixel[1]<100&&pixel[2]<100
                                    };
                                    let actual=dark(&top);let expected=dark(&reference);
                                    ink+=usize::from(actual||expected);changed+=usize::from(actual!=expected);
                                }}
                                anyhow::ensure!(ink>60&&changed*100<=ink*8+300,
                                    "RTL pixels must match the independent legacy unjoined-glyph reference: changed={changed}, ink={ink}");
                            }
                            if fixture["expected_formatted_rows"].as_bool().unwrap_or(false) {
                                let result=&report["results"][0];
                                let lines=result["lines"].as_array().expect("native source rows");
                                anyhow::ensure!(lines.len()==2&&lines[0]["range"]==serde_json::json!({"start":0,"end":2})
                                    &&lines[1]["range"]==serde_json::json!({"start":3,"end":9})
                                    &&lines[1]["ellipsis"]==true&&result["positions"][1]["rect"].is_null(),
                                    "native ellipsis must preserve visible original ranges and hide later source carets: {result:?}");
                                for line in lines {
                                    let rect=&line["rect"];
                                    anyhow::ensure!((rect["x"].as_f64().unwrap()+rect["w"].as_f64().unwrap()/2.-100.).abs()<0.01,
                                        "each native row must be centered independently: {line:?}");
                                }
                                let center=&report["results"][1]["lines"][0];
                                anyhow::ensure!(center["rect"]["y"]==128.&&center["rect"]["h"]==24.,
                                    "the complete one-row block must be vertically centered: {center:?}");
                                let red=image.pixels().filter(|pixel|pixel[0]>180&&pixel[1]<80&&pixel[2]<80).count();
                                anyhow::ensure!(red>40,"native style-run red glyphs missing: {red}");
                            }
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
