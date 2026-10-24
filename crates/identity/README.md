# nooboard-identity

独立后台服务，拥有 APP 身份私钥，提供公开身份、签名和用途隔离的本地加解密能力。Core 持有服务所有者并向 Network 注入签名适配器；Network 和 Terminal 都不依赖本 crate。配置、SSH 密码输入和密文持久化仍属于 Core。

## 使用接口

`IdentityService` 是不可克隆的生命周期所有者，`Identity` 是可克隆的请求句柄。服务需要 Tokio runtime，私钥与加解密操作位于独立阻塞 worker；异步调用通过有界队列提交。

```rust,no_run
use nooboard_identity::{IdentityConfig, IdentityOptions, IdentityService, SecretBytes, SecretContext};

# async fn example(profile: String) -> nooboard_identity::Result<()> {
let (mut service, identity) = IdentityService::start(
    IdentityConfig::System { profile },
    IdentityOptions::default(),
).await?;

let binding = SecretContext {
    purpose: "terminal.ssh-auth".into(),
    // Core 应使用无歧义编码，绑定实际配置 UUID、字段和格式版本。
    associated_data: b"example-config-uuid/password/v1".to_vec(),
};
let sealed = identity.seal(binding.clone(), SecretBytes::new(b"example-secret".to_vec())).await?;
// Core 可序列化 sealed 保存；本服务不持久化该密文。
let plaintext = identity.open(binding, sealed).await?;
assert_eq!(plaintext.expose_secret(), b"example-secret");
drop(plaintext);

service.shutdown().await?;
# Ok(())
# }
```

| 接口 | 契约 |
| --- | --- |
| `IdentityService::start(config, options)` | 身份加载、校验或首次持久化成功后才返回所有者和句柄 |
| `public_identity` | 公开证书、稳定设备 ID、证书指纹和支持的签名算法，不含展示名称 |
| `status` | 当前服务状态及公开身份 |
| `sign` | 使用指定算法签名原始消息；算法内规定的哈希由实现执行 |
| `sign_blocking` | 供 rustls 等同步协议回调使用；队列满立即 Busy，等待有超时 |
| `seal` / `open` | 应用秘密的认证加密与解密，不导出身份私钥或派生密钥 |
| `shutdown(&mut self)` | 停止接纳，拒绝未执行请求，等待当前操作和材料释放；超时保留所有者以便继续等待 |

同步签名只调用独立 worker，不能回调等待请求方的异步任务。Core 的适配器将自身持有的 Identity 句柄转换为 Network 定义的 `IdentitySigner`，TLS 层取得签名结果而非私钥。现支持 ECDSA P-256/P-384、Ed25519 和 RSA-PSS 的 TLS 1.3 签名方案；实际可用方案由已加载的密钥决定。

释放句柄不会关闭服务；释放所有者会请求停止。需要确认清理结果时显式等待 `shutdown`。已经开始的操作可能完成并返回结果，尚未执行的请求返回 Stopped。身份服务必须晚于 Network 及以后接入的 Terminal 等使用者关闭。被调用方应持有完整生命周期，不能在仍需签名时提前停止 Identity。

## 现有身份迁移

此次迁移保留原有存储位置和字节格式，不进行密钥轮换或另建凭据：

- 系统凭据 service 名仍是 `nooboard.identity.v1`，account 仍为 Core 传入的 profile。
- 内容仍是四字节大端证书长度、DER 证书和 PKCS#8 私钥。读取时校验结构以及证书与私钥匹配。
- 设备 ID 仍是证书 SPKI 的 SHA-256 十六进制值；证书指纹仍是完整 DER 证书的 SHA-256。
- 同 profile 的跨进程创建继续使用原锁目录、文件名和排他锁，兼容旧版启动协调。
- 只有明确不存在条目才生成身份并写入。拒绝访问、损坏、保存失败和锁超时均报错，不替换原条目、不退回临时身份。

这是一项服务归属迁移，用户不需要重新配对。Core 的公开设备信任记录和网络协议格式保持不变。普通测试不会打开、迁移或重写真实系统凭据；旧格式兼容通过合成的旧格式材料与替代凭据读写函数验证。

`IdentityConfig::Ephemeral` 仅供明确的临时用途和测试。`native-credentials` 是本 crate 的可选默认 feature；关闭后请求 System 会返回 UnsupportedStorage，Ephemeral 仍可使用。Network 已不再提供身份生成模式或 `native-credentials` feature。

## 加密格式与职责

`SealedSecret` v1 使用 HKDF-SHA256 从已加载的 PKCS#8 私钥材料派生用途密钥，以 XChaCha20Poly1305 加密。每次 seal 生成新的 24 字节随机 nonce；密文附带认证标签。派生 info 与认证附加数据有独立版本前缀和长度编码，绑定 `purpose` 与调用方的 `associated_data`。实现依据：[HKDF RFC 5869](https://www.rfc-editor.org/rfc/rfc5869.html)、[RustCrypto XChaCha20Poly1305](https://docs.rs/chacha20poly1305/0.10.1/chacha20poly1305/)。

Core 定义应用配置中的 `EncryptedSecret`、字段含义和存储方式。配置 UUID 或字段改变时，Core 需用旧绑定解密，再用新绑定重新加密。Identity 不推断配置结构，也不通过配置查询寻找密码。不同身份、用途、配置绑定或遭篡改的内容不能通过认证；未知加密版本明确拒绝，不猜测兼容格式。

密文绑定 APP 身份，并非不可复制的硬件身份。只更换证书而保留相同 PKCS#8 密钥材料不会改变派生结果；未来若轮换或重新编码身份私钥，必须单独设计秘密迁移，不能静默重建身份。`SecretBytes` 不可序列化，Debug 隐去内容，拥有的明文与私钥缓冲在释放时清零；不承诺所有第三方密码库的内部副本都可由本 crate 清除。

## 配置与验证

时限、队列容量和请求大小上限统一在 [config/defaults.json](config/defaults.json)。可用 `IdentityOptions::from_json` 加载调用方保存的配置，启动后固定。本 crate 不读取 Core 的 Storage，也不创建后台配置文件。

```sh
cargo test -p nooboard-identity --all-targets --locked
cargo test -p nooboard-identity --all-targets --no-default-features --locked
cargo clippy -p nooboard-identity --all-targets --all-features --locked -- -D warnings
```

独立测试覆盖旧格式恢复、签名验证、证书重新签发后的稳定 ID、加解密与用途绑定、篡改拒绝、错误凭据不覆盖、单线程 runtime 同步签名和关闭。Core 的真实回环配对/TLS/传输测试验证正式注入路径。系统凭据弹窗、Linux Secret Service 会话和其他平台仍需原生验收。
