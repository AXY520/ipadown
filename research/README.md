# ipadown 项目前期调研（2026-09-24，同日修订 v3）

**产品定位（v3 已明确）**：对标闭源工具 ipaDown（吾爱破解 @苍井灰灰 发布）——登录自己的 Apple ID，下载 App Store **历史版本** IPA（含已下架应用），**不修改 IPA 任何字节**，通过**局域网二维码 OTA** 安装到 iPhone。因保留 Apple 原版 FairPlay 签名，安装后**永久不过期**（详见 04 篇 §2 与 05 篇 §1）。
目标平台：Linux 桌面；实现语言：**Rust**。

分报告：

1. [01-ipa-download.md](./01-ipa-download.md) — IPA 下载方案（ipatool / ApplePackage(Asspp) / ipatool-py 等）
2. [02-apple-id-signing.md](./02-apple-id-signing.md) — 重签名方案（zsign / AltSign / anisette / 证书获取；§5 有"7 天过期适用范围"澄清）
3. [03-linux-install-toolchain.md](./03-linux-install-toolchain.md) — 有线安装工具链（go-ios / pymobiledevice3 / libimobiledevice）
4. [04-installation-modes.md](./04-installation-modes.md) — 安装方式总论：有线 / 无线配对 / **在线 OTA（itms-services）**；签名有效期对照表
5. [05-rust-stack.md](./05-rust-stack.md) — **Rust 技术栈可行性**（idevice / apple-codesign / 协议层移植）；原版签名安装边界条件

## 推荐总体架构（v2）

```
用户 Apple ID（建议小号）
   │
   ▼
[下载]  majd/ipatool 二进制（协议层可参考 Lakr233/ApplePackage）
   │     auth login(+2FA) → purchase(免费App) → download
   ▼
官方 DRM IPA ──────────────┐
   │                        │ 路径 A：原版安装（不过期）
   │ 路径 B：重签名          │ 要求设备 App Store 账号已拥有该 App
   ▼                        │
[签名] iloader/AltServer-Linux│
   取 p12+mobileprovision    │
   （anisette: docker 自建   │
   dadoum/anisette-v3-server）│
   → zhlynn/zsign 重签       │
   │                        │
   ▼                        ▼
        [安装] 双通道（对 A/B 产物均适用）
        ├─ 有线：go-ios（iOS 17+ 先 ios tunnel start）
        │        ideviceinstaller 兜底（iOS ≤16）
        └─ 在线：局域网 HTTPS + manifest.plist
                 → itms-services://?action=download-manifest&url=...
                 （证书方案：自有域名通配符证书 + DNS 指到局域网 IP，
                   抄 Asspp 的 app.localhost.qaq.wiki 作业）
```

## 最终范围（v3，用户确认）

- **MVP 只做三件事**：① Apple ID 登录（含 2FA）+ 搜索 + 历史版本枚举/下载；② 局域网 HTTPS + manifest.plist + 二维码 OTA 安装；③ 原版签名，零修改，零重签。
- **移出范围**：重签名（zsign/apple-codesign）、anisette、证书获取、有线安装——全部砍出 MVP；如有需要作为 v2+ 的独立功能再议（调研结论保留在 02/03 篇备查）。

## 技术方案（已定）

- **语言/栈**：Rust workspace。商店协议层参照 majd/ipatool（Go）与 Lakr233/ApplePackage（Swift，其 `VersionFinder` 是历史版本枚举的最干净一手参照）自行移植（REST + plist）；OTA 用 axum + rustls；二维码用 `qrcode` crate。
- **唯一基础设施依赖**：一个域名 + 通配符证书（Let's Encrypt DNS-01），DNS 指到主机局域网 IP——与 Asspp（`app.localhost.qaq.wiki`）/ipaDown 同款手法，用于让 itms-services 的 manifest URL 满足 Apple 的"受信 HTTPS"要求。
- **设备侧要求**：iPhone 的 App Store 账号需拥有该 App（免费 App 点一次"获取"即可）；iOS 18+ 手动安装需在 UX 提示"装完重启一次设备"。
- **风控提示**：建议用户用小号 Apple ID（Asspp README 明确警告封号风险）。

## 里程碑

- M1：CLI 登录 + 搜索 + 版本列表 + 下载（协议层闭环）
- M2：局域网 OTA + 二维码安装（产品闭环，可对标 ipaDown）
- M3：GUI/TUI 打磨、会话持久化与账号池（参考 Asspp 数据模型）
