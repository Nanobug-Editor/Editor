---
title: HTTP、密钥与私有状态
description: 有界云查询、系统凭据、私有目录与可释放计时器。
section: sdk
order: 19
alternate: /en/sdk/cloud/
---

# HTTP、密钥与私有状态

`network.http` 1.0.0、`storage.secrets` 1.0.0 和 `lifecycle.timers` 1.0.0 是稳定能力。
目录操作、缓存和临时区域要求稳定的 `storage.private` 1.2.0。最低宿主支持
`protocol = 7`、`api.base = ^1` 和这些能力版本。
适用统一的[兼容性与弃用规则](/zh-cn/sdk/#稳定性与能力兼容)。必需能力不可用时拒绝插件包；
可选能力不可用时关闭对应功能或使用插件自己的回退。这些能力不提供浏览器 OAuth 或 AI API。

```json
{
  "protocol": 7,
  "api": {"base": "^1", "required": {
    "network.http": "^1", "storage.secrets": "^1",
    "lifecycle.timers": "^1", "storage.private": "^1.2"
  }},
  "permissions": ["network.http", "network.http:https://api.example.test", "secrets", "storage"]
}
```

## HTTP 请求与流式响应

`http::start(Request)` 返回实例拥有的流句柄。请求支持方法、头及可选字节体，独立于图片加载和
原生进程。`http::read(&handle)` 每次取出一个事件并释放队列容量；`None` 表示待处理。
事件为 `Headers { status, headers }`、`Chunk { bytes }`、`Finished { bytes }`、`Failed { error }`。
非 2xx 状态码属于普通响应元数据。响应头先于字节块，唯一终态事件封闭流；继续读取返回
`InvalidState`。收到终态后调用 `api::guest::close_resource` 释放句柄。

```rust
let mut request = plugin_protocol::http::Request::get("https://api.example.test/query");
request.method = "POST".into();
request.body = "hello 世界😀".as_bytes().to_vec();
let stream = plugin_protocol::http::start(request)?;
// 待处理时用可释放计时器重试；每次调用只消费一个有界事件。
let event = plugin_protocol::http::read(&stream)?;
```

| 限额 | 数值 |
| --- | --- |
| 请求体 | 256 KiB |
| 请求头 | 64 项，名称与值合计 16 KiB |
| 响应头 | 128 项，完整线上头块最多 32 KiB |
| 响应体 | 请求限额 1 字节–16 MiB，默认 1 MiB |
| 分块与队列 | 每块最多 32 KiB，队列最多八个事件 |
| 截止时间 | 从接受起算 100–120,000 ms，默认 30,000 ms |
| 重定向 | 0–5 次，默认 3 次 |
| 生产者 | 宿主 HTTP 配置共享最多四个并发工作线程 |

方法由大写 ASCII 字母或连字符组成，最多 32 字节；禁止 `CONNECT` 与 `TRACE`。
头值接受可打印 ASCII 和水平制表符；Unicode 内容应编码在请求体或 URL 中。
Host、消息分帧、连接、升级和代理认证头由宿主管理。URL 必须为绝对 HTTP(S)，最多
4,096 字节，禁止用户信息、`@`、片段及控制字符。无效请求在分配资源前失败。
即使 Content-Length 缺失或不可信，仍执行字节限额。慢消费者暂停有界生产者，并继续消耗原截止时间。

同时批准 `network.http` 和精确规范化 origin 权限；非默认端口必须包含在 origin 中。
不支持通配目标。每次重定向在连接前检查下一 origin。禁止 HTTPS 降级到 HTTP。
跨 origin 重定向清空所有调用方请求头，并拒绝转发非空请求体。303，或 POST 的 301/302，
先改为 GET 并清空请求体，再执行此检查。相对重定向遵循相同规则。读取密钥不授予网络权限。

宿主选择代理及 TLS 根证书；插件不能关闭证书链或主机名验证，也不继承宿主代理环境变量。
Windows 宿主接受显式 `NANOBUG_HTTP_PROXY` 设置，使用 HTTP(S) CONNECT 代理；未设置时直接连接。
嵌入宿主未提供显式根证书集时使用标准 WebPKI 根证书。代理配置不受支持、离线连接、DNS 或 TLS
失败返回安全的错误类别，不回显 URL 或携带凭据的数据。

HTTP 句柄上的 `CancelRequest` 停止等待并关闭套接字，下一次读取得到终态
`Failed(Cancelled)`。句柄继续归实例所有，直到 `CloseResource` 释放。直接关闭会取消，
不保留可读取终态。套接字关闭也中断阻塞的 TLS、CONNECT 与响应体 IO；套接字 IO 最多每
50 ms 检查撤销，不延长原截止时间。
OS DNS 本身是同步调用：消费者立即观察到取消或截止时间，但已经运行的 DNS 调用可能继续
占用四个线程中的一个，直到 OS 返回。撤销后不会启动迟到连接，也不会创建额外无界 DNS 线程。

## 系统密钥

协商 `storage.secrets ^1` 并单独批准 `secrets` 权限。SDK 提供 `secrets::read(key)`、
`write(key, bytes)` 和 `delete(key)`。键非空、最多 128 个 UTF-8 字节，不含控制字符；
值为 1–2,560 字节。缺失的键读取为 `None`，删除为幂等操作。Windows 使用当前 OS 用户的
持久 Generic Credentials，以宿主配置、插件 ID 和键划分命名空间。其他插件即使拥有
`secrets` 也不能指定或读取该命名空间；不提供凭据枚举。

其他平台返回 `UnsupportedOperation`；Windows 凭据设施失败返回 `OperationFailed`。
两种情况均不写入明文文件。写入和删除要求已经提交的激活，准备、迁移、失败候选及受委托服务
不能修改密钥。禁用和撤销信任仅移除访问权，保留存储的凭据，以供以后授权激活；显式删除才移除凭据。

使用密码[原生输入](/zh-cn/sdk/interaction/)接收 Token。返回的密钥是敏感应用数据，
不得放入普通设置、日志、视图或快照。HTTP 与密钥请求的 Debug 输出省略敏感载荷。
公开服务调用保留调用方网络权限，包括目标 origin；不能访问提供者私有文件或密钥，也不能借用
提供者更强的授权。提供者不得转交自己的 Token。

## 私有目录、缓存与临时字节

批准 `storage`。已有 `OpenData`、`ReadFile`、`WriteFile` 继续有效。协商
`storage.private ^1.2` 后使用 `storage::open(Area::Data | Cache | Temporary)` 和
`storage::operate(Operation)`；同一拥有者句柄支持文件读写。`Stat`、`List`、
`CreateDirectory`、`Rename` 和 `Delete` 管理相对路径。List 返回排序后的 1–128 项页面；
把上一页最后名称作为 `after`，并检查 `has_more`。元数据提供名称、目录标记及文件字节数；
目录字节数为零。CreateDirectory 要求父目录存在，Rename 不覆盖目标。删除非空目录必须
显式指定 `recursive`。禁止删除或改名区域根。

所有区域共享清单字节配额，每文件最多 1 MiB，最多 1,024 项、合计 64 KiB 名称字节、深度 16。
路径最多 4,096 字节。修改前预留条目与名称限额，空目录也计数。
拒绝绝对路径、穿越、Windows 设备、备用数据流、反斜杠、符号链接与 reparse point。
初始化保留现有事务的根目录直接子文件暂存写入及最多 64 个暂存文件；目录管理要求已提交激活。

数据和缓存持久化在工作区外的宿主管理私有存储中，并参与
[私有数据事务替换](/zh-cn/sdk/migration/)。实例退役时删除临时字节。原生清理失败会报告原因，
退役实例的访问权仍保持撤销。其他插件、作用域或先前实例的句柄均无效。

## 可释放计时器与 debounce

`timers::start(delay_ms, repeat)` 返回实例拥有的计时器；`reset(&handle, delay_ms)`
替换下一截止时间。延迟范围 10 ms–一天，每实例最多 32 个计时器。到期时发送
`Notification::Timer { handle, sequence }`。重复计时器合并错过的 tick，从实际送达时刻安排
下一 tick；每次送达增加序号。每轮 poll 最多分派八个最早到期的计时器，使每个到期计时器得到执行机会。
送达依赖宿主 poll，不保证硬实时。单次计时器在回调前释放，触发后不能重置。

实现 debounce 时保留一个单次计时器，在触发前对每次刷新意图重置它。
取消或替换意图时关闭待触发的 debounce 句柄，避免它重新开始查询。
周期 HTTP 轮询保留一个重复计时器，在结束或取消时与流一起关闭。
禁用、撤销信任、更新退役和激活失败都会撤销流、计时器及其服务来源授权。
恢复以新实例身份还原逻辑私有数据，不重放外部 HTTP 副作用或恢复套接字。
