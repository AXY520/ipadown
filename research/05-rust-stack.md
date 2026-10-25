# Rust 技术栈可行性——补充调研（2026-09-24）

一句话总结：**Rust 全链路可行**。设备通信有生产级纯 Rust 库 `jkcoxson/idevice`，签名有 `apple-codesign` 纯 Rust crate，商店协议层需自行移植（参照 ipatool/ApplePackage，纯 REST+plist），OTA 服务是普通 Web 后端活。

## 1. 原版签名安装（路径 A）是否可用？——可以，附边界条件

「FairPlay 签名跟随账号 license、永久不过期」属实，且已被 Asspp 量产验证：它下载的 IPA 原封不动、通过 OTA 装回设备（来源：https://github.com/Lakr233/Asspp 的 Installer 模块，本调研 04 篇 §1.4）。使用条件：

1. **IPA 必须逐字节原样**：任何修改（bundle id、图标、注入、去广告）都会破坏 Apple 签名，一旦改动就必须走重签路径（B），并接受其有效期约束。
2. **设备的 App Store 账号需拥有该 App**：下载用哪个 ID 取得 license，目标设备的「媒体与购买项目」就应登录该 ID（或该设备账号已获取过此 App）；否则首启 FairPlay 校验会要求输入购买账号密码甚至失败。个人自用（同 ID）完全没问题。
3. **全部功能可用**：原版签名保留推送/iCloud/内购等 entitlements——这是相对重签路径的显著优势。
4. **安装通道注意**：原版 IPA 不含 `embedded.mobileprovision`（商店包的 profile 由商店侧下发），有线 installd 直装商店包是否可行**未经一手验证**；已验证的可靠通道是 **OTA**（Asspp 模式）。因此路径 A 建议 OTA 优先，有线通道留待真机实验确认。

## 2. Rust 各链路现状（逐项核实）

| 链路 | Rust 方案 | 状态 |
|---|---|---|
| 设备通信（配对/隧道/安装） | `jkcoxson/idevice`（MIT，2026-09-15 仍活跃，515★；已被 StikDebug、CrossCode、Protokolle 等量产应用采用，"部署在全球数万台设备上"） | ✅ 核心可用。feature 涵盖：`usbmuxd`、`pair`、`core_device_proxy`（iOS 17+ 隧道）、`xpc` over RSD、`installation_proxy`（安装）、`misagent`（profile 管理）、`mobile_image_mounter`（DDI）、`tcp`（网络设备） |
| 重签名（路径 B） | `apple-codesign`（indygreg/apple-platform-rs，2026-09-19 活跃，858★；纯 Rust Mach-O/Bundle 签名 + CMS，CLI 为 `rcodesign`） | ✅ 可替代 zsign；改 entitlements/bundle id 配合 `plist` crate 完成 |
| 商店协议（登录/purchase/download） | **无现成实现，需移植**：参照 majd/ipatool（Go）与 Lakr233/ApplePackage（Swift）源码；本质是 REST + plist 编解码（`plist` crate）+ `reqwest` | ⚠️ 自研工作量集中在此，但协议薄、有一手参照 |
| OTA 在线安装 | `axum`/`tokio` + `rustls` 起 HTTPS 静态服务，生成 manifest.plist | ✅ 普通 Web 活 |
| anisette（仅路径 B 证书获取需要） | 无成熟 Rust 实现；docker 调 `dadoum/anisette-v3-server`（语言无关） | ✅ 外部进程 |
| 兜底 | Rust `std::process` 调 go-ios / zsign 二进制 | ✅ 永远可用 |

注意：`idevice` 处于 0.1.x 阶段，README 明确"0.2.0 前每个版本都有 breaking changes"，Cargo.toml 需**钉死精确版本**（来源：https://github.com/jkcoxson/idevice README「State」一节）。

## 3. 推荐 Rust 项目结构

```
ipadown (workspace)
├─ crates/store     # 商店协议：auth(2FA)/search/purchase/download/versions（移植 ipatool）
├─ crates/device    # 设备层：封装 idevice crate（配对/tunnel/install），兜底调 go-ios 二进制
├─ crates/sign      # 路径 B：apple-codesign 重签 + profile 管理；证书获取先外挂 iloader 流程
├─ crates/ota       # axum HTTPS + manifest.plist + itms-services 链接/二维码
└─ src/main.rs      # CLI（clap）：download / sign / install --usb|--ota
```

里程碑建议：M1 原版下载+OTA 安装（最快闭环，无证书难题）→ M2 有线安装 → M3 重签路径。
