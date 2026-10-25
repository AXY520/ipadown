# 调研 02：用个人 Apple ID（免费开发者账号）给 IPA 重签名

> **一句话总结**：免费 Apple ID 完全可以给任意 IPA 重签名并装到自己的 iPhone 上（无需越狱、无需 Mac），整条链路（证书申请 → profile 生成 → Mach-O 重签 → 安装）在 Linux 上都有活跃维护的开源实现；代价是 Apple 施加的硬限制——签名应用 7 天过期需反复续签、每周最多注册 10 个 App ID、同时只能激活 3 个 sideload 应用、设备 UDID 必须先注册进 profile。

---

## 1. 免费 Apple ID 签名的核心限制

### 1.1 应用 7 天过期

- 免费 provisioning（Apple 官方称 "Personal Team"）生成的 **provisioning profile 一周后过期，签名证书（signing identity）一年后过期**。
  来源（Microsoft Learn《Free provisioning for Xamarin.iOS apps》，对 Apple 行为的官方描述）：https://learn.microsoft.com/en-us/xamarin/ios/get-started/installation/device-provisioning/free-provisioning
- SideStore 官方 FAQ 称之为 "normal 7-day development period"，因此 SideStore/AltStore 需要在后台周期性 "refresh"（重新签名安装）来阻止过期。
  来源：https://docs.sidestore.io/docs/faq
- 付费 $99/年的 Apple Developer Program 账号则没有此限制，签名有效期 365 天，且不受 3 个应用的数量限制。
  来源：https://docs.sidestore.io/docs/faq

### 1.2 每周最多 10 个 App ID

- 免费账号**同时只能注册 10 个 App ID，每个 App ID 一周后过期自动释放**（App 内每个 extension 也要单独占一个 App ID 名额）。
  来源（AltStore 官方 FAQ《App IDs》）：https://faq.altstore.io/altstore-classic/app-ids
- SideStore FAQ 的表述："only 10 different apps may be installed in a week (Referred to as App IDs)"。
  来源：https://docs.sidestore.io/docs/faq

### 1.3 同时最多 3 个 sideload 应用处于激活状态

- "due to Apple's restrictions only 3 sideloaded apps can be active at a time"；超出的应用必须先 deactivate（备份数据并移除）才能再装新的。
  来源（AltStore 官方 FAQ《Activating Apps》）：https://faq.altstore.io/altstore-classic/activating-apps
- SideStore FAQ 补充：免费账号下 SideStore **自身就占掉 3 个名额中的 1 个**；iOS 18 db5/18.0.1 及以下可用 SparseRestore 漏洞绕过 3 应用限制（但绕不过 10 个 App ID 限制）。
  来源：https://docs.sidestore.io/docs/faq

### 1.4 设备 UDID 必须先注册

- Apple 官方文档：创建 development/ad hoc provisioning profile 的前提是先注册设备，注册时需要设备名和 **UDID（Unique Device Identifier）**；使用 Xcode 自动签名时 Xcode 会自动注册连接的设备（第三方工具链则通过开发者 API 自动完成同样的事）。
  来源：https://developer.apple.com/help/account/devices/register-a-single-device/
- 实操层面：签名所用的 profile 里必须包含目标设备的 UDID，否则安装时报 "integrity could not be verified"。
  来源（SignTools FAQ）：https://github.com/SignTools/SignTools/blob/master/FAQ.md

### 1.5 能力（capabilities / entitlements）受限

- 免费 provisioning 无法使用大多数 App 服务类 entitlement：Apple Pay、Game Center、iCloud、In-App Purchase、推送通知、Wallet 等均不可用；且必须使用 explicit App ID（不能用 wildcard），bundle id 一旦用过不能复用。
  来源：https://learn.microsoft.com/en-us/xamarin/ios/get-started/installation/device-provisioning/free-provisioning
- 推论：重签第三方 IPA 时，原应用里凡依赖上述 entitlement 的功能（推送、iCloud 同步、内购恢复等）都会失效。

---

## 2. 签名工具链（Linux 可用性逐项确认）

### 2.1 zsign —— Linux 上最成熟的 IPA 重签工具

- 仓库：https://github.com/zhlynn/zsign （MIT，2019 年创建，~1.9k stars，最近 push：2026-08-21，仍活跃维护）
- 定位：跨平台（macOS/Linux/Windows/Android/FreeBSD）的 codesign 替代品，支持 iOS 12+，不需要 Xcode/macOS；支持 `.ipa`、Mach-O、`.app` 重签。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md
- 关键功能（README 一手确认）：
  - p12 + mobileprovision 签名：`zsign -k dev.p12 -p <密码> -m dev.prov -o output.ipa demo.ipa`
  - ad-hoc 签名（`-a`）；`.zsign_cache` 增量缓存加速二次签名
  - dylib 注入/移除：`-l`（`LC_LOAD_DYLIB`）、`-w`（`LC_LOAD_WEAK_DYLIB`）、`-D` 移除、`-P` 连扩展一起注入
  - 改 bundle id（`-b`）、显示名（`-n`）、版本号（`-r`）、图标（`-I`）、自定义 entitlements（`-e`）
  - 多 profile（每个扩展一个 `-m`）、删扩展（`-E`）、删 watch app（`-W`）、OCSP 证书校验（`-C`）、签完直接 `-i` 调 ideviceinstaller 安装
- Linux 构建：`apt install git g++ pkg-config libssl-dev && cd build/linux && make`（README 一手确认）。

### 2.2 ldid —— 伪签名/entitlements 的底层角色

- 仓库：https://github.com/ProcursusTeam/ldid （saurik 原版的 Procursus 维护分支，AGPL-3.0，最近 push：2026-07-24，活跃）
- 官方 man page 的一手定义：ldid "adds SHA1 and SHA256 hashes to a Mach-O file so that they can be run on a system that has **validation, but not signature verification**"——即它的本职是为**越狱设备**做伪签名（ad-hoc），`-S file.xml` 把 entitlements 嵌进 Mach-O；`-K file` 也能用 p12 做真实签名。
  来源：https://github.com/ProcursusTeam/ldid/blob/master/docs/ldid.1
- 与免费 Apple ID 方案的关系：AltStore 的签名库 **AltSign 直接把 ldid 源码作为子模块内嵌**（podspec 中的 `ldid` subspec）来执行 Mach-O 层面的签名；但在非越狱场景下必须用真实 p12 做 CMS 签名，纯 ldid 伪签名装不上未越狱设备。
  来源：https://github.com/rileytestut/AltSign/blob/master/AltSign.podspec

### 2.3 AltStore / SideStore 体系

**AltSign（Riley Testut，AltStore 作者）**
- 仓库：https://github.com/rileytestut/AltSign （Swift/iOS 框架，最近 push：2026-09-17，活跃；README 缺失，以 podspec/源码为准）
- 职责：管理 Apple 开发者账号、重签应用的 iOS 端库；内嵌 OpenSSL + ldid + corecrypto。
  来源：https://github.com/rileytestut/AltSign/blob/master/AltSign.podspec
- Linux 移植版存在但已停更：https://github.com/NyaMisty/AltSign-Linux （最近 push：2022-04）

**SideSign（SideStore 新一代签名库）**
- 仓库：https://github.com/SideStore/SideSign （最近 push：2026-09-18，非常活跃，SideStore 已将其作为官方 submodule）
  来源：https://github.com/SideStore/SideStore/blob/develop/.gitmodules
- README 一手确认的能力：
  - GSA（GrandSlam）SRP-6a 登录认证（GSACryptoKit）
  - 本地生成 2048 位 RSA 私钥 + PKCS#10 CSR，提交给 Apple 开发者 API 换取 iOS Development 证书，p12 导入导出解析（CodeSignKit）
  - 调开发者 API 注册 App ID、App Group、设备 UDID，生成并下载 development provisioning profile
  - Mach-O 重签、重写 `application-identifier`/`team-identifier`/`keychain-access-groups` 等 entitlements、嵌入 embedded.mobileprovision、递归签扩展和 framework、重打包 IPA
  来源：https://github.com/SideStore/SideSign/blob/main/README.md

**AltServer / AltServer-Linux / iloader（桌面端签名安装器）**
- AltServer-Linux：https://github.com/NyaMisty/AltServer-Linux （AGPL-3.0，~1.2k stars，最近 push：2025-03，半停滞但可用）
  - 用法一手确认：`./AltServer -u <UDID> -a <AppleID> -p <密码> app.ipa` 即可完成「登录 Apple → 申请证书/profile → 签名 → 经 usbmuxd 安装」全流程；支持 `ALTSERVER_ANISETTE_SERVER` 环境变量指定自建 anisette 服务器（默认用公共服务器）。
  来源：https://github.com/NyaMisty/AltServer-Linux/blob/new/README.md
- iloader：https://github.com/nab138/iloader （MIT，~3.6k stars，最近 push：2026-09-23，非常活跃；SideStore 官方文档推荐的现行安装器，有 Windows/macOS/Linux 版本）
  - 功能：给设备装 SideStore 或任意 IPA、登录 Apple ID、管理 pairing 文件、**查看并吊销开发证书和 App ID**。
  来源：https://github.com/nab138/iloader/blob/main/README.md 、https://docs.sidestore.io/docs/installation/prerequisites

**minimuxer / em_proxy（设备端安装通道，SideStore 用）**
- minimuxer：https://github.com/SideStore/minimuxer （最近 push：2026-09-20）——跑在 iOS 设备上的 lockdown muxer 库，封装 libimobiledevice，负责应用安装/卸载、DDI 挂载、AFC 文件操作，通过本机 loopback VPN 或远程端点与设备通信。
  来源：https://github.com/SideStore/minimuxer/blob/develop/README.md
- em_proxy：https://github.com/SideStore/em_proxy （最近 push：2026-08-26）——Rust 写的透明 TCP 代理，伪装成本机 WireGuard 服务端（基于 boringtun），改写包的源/目的地址以绕过 iOS 的 loopback 限制；这正是 SideStore 能"在手机上自己给自己装应用"的关键hack。
  来源：https://github.com/SideStore/em_proxy/blob/develop/README.md

**anisette 数据问题与 Linux 解法**
- 什么是 anisette：每次安装/刷新应用时，向 Apple 服务器认证都需要的一组设备指纹数据，由伪装成 Mac 的机器生成；SideStore 从 anisette 服务器获取，账号信息不会发给服务器。
  来源：https://docs.sidestore.io/docs/advanced/anisette
- 风险：多人共用同一个 v1 anisette 服务器会触发 Apple 风控导致**账号被锁**（这也是官方建议用小号的原因）；SideStore 0.4.0+ 支持 v3 协议后大幅缓解。
  来源：https://docs.sidestore.io/docs/advanced/anisette
- Linux 原生方案：
  - **Dadoum/Provision**：https://github.com/Dadoum/Provision —— 在 Linux 上模拟苹果设备的库，原理是加载 Android 版 Apple Music APK 里的 `libstoreservicescore.so` 和 `libCoreADI.so` 两个苹果官方库来生成本机 ADI/anisette 数据；提供 anisette_server（供 AltServer-Linux 使用）和 retrieve_headers。ADI 数据存于 `~/.adi/adi.pb`，泄露等于设备身份泄露。
    来源：https://github.com/Dadoum/Provision/blob/main/README.md
  - **Dadoum/anisette-v3-server**：https://github.com/Dadoum/anisette-v3-server （最近 push：2026-09-23，活跃）——docker 一键部署，兼容 v1/v3 协议，官方说可替代 SideStore/omnisette-server。
    来源：https://github.com/Dadoum/anisette-v3-server/blob/master/README.md
  - SideStore/omnisette-server：https://github.com/SideStore/omnisette-server —— 已被官方标注 "essentially replaced by Dadoum's anisette-v3-server"。底层是 Rust 库 https://github.com/SideStore/apple-private-apis （omnisette）。
    来源：https://github.com/SideStore/omnisette-server/blob/main/README.md
  - SideStore 新版（经 SideSign 的 AnisetteKit）已支持**设备端本地生成 anisette**，可不依赖外部服务器。
    来源：https://github.com/SideStore/SideSign/blob/main/README.md
- "Docker 里跑 macOS"思路：https://github.com/sickcodes/Docker-OSX （~53k stars，KVM 虚拟化跑真 macOS）——可行但重，仅当需要 Xcode 官方工具链（如 SignTools builder）时才值得。

### 2.4 SignTools —— 现成的签名 Web 服务

- 仓库：https://github.com/SignTools/SignTools （AGPL-3.0，~2.2k stars，最近 push：2026-09-21，活跃）
- 架构（README 一手确认）：**service（可跑在任意 Linux 服务器，提供 Web 上传/签名/下载/OTA 安装界面）+ builder（一台 macOS，用 Apple 官方 codesign 执行签名）**；支持注入 tweak、配置 entitlements、多签名 profile、免电脑（初次配置后纯手机操作）。
  来源：https://github.com/SignTools/SignTools/blob/master/README.md
- FAQ 中的重要一手知识：
  - **Wildcard profile**（`TEAM_ID.*`）能签任意 bundle id 但带不了 app groups/iCloud 等 entitlements；**Explicit profile**（`TEAM_ID.app1`）只能签一个 id 但 entitlements 完整。
  - 某些应用（unc0ver、DolphiniOS 等）需要 `get-task-allow`（调试 entitlement），必须用 Development 证书。
  - "integrity could not be verified" 错误的排查方向：profile 无效或设备 UDID 未注册进 profile。
  来源：https://github.com/SignTools/SignTools/blob/master/FAQ.md

### 2.5 ios-signer-service

- 就是 SignTools 的前身/旧名：`SignTools/ios-signer-service` 仓库内容与 SignTools 完全相同（README 已同步为新项目），实际以 SignTools 为准。
  来源：https://github.com/SignTools/ios-signer-service/blob/master/README.md

### 2.6 活跃度速览（2026-09-24 查询 GitHub API）

| 项目 | 最近 push | License | 状态 |
|---|---|---|---|
| zhlynn/zsign | 2026-08-21 | MIT | 活跃 |
| ProcursusTeam/ldid | 2026-07-24 | AGPL-3.0 | 活跃 |
| rileytestut/AltSign | 2026-09-17 | MIT | 活跃 |
| SideStore/SideSign | 2026-09-18 | — | 非常活跃（新项目） |
| SideStore/minimuxer | 2026-09-20 | — | 活跃 |
| SideStore/em_proxy | 2026-08-26 | — | 活跃 |
| NyaMisty/AltServer-Linux | 2025-03-16 | AGPL-3.0 | 半停滞（可用） |
| Dadoum/Provision | 2025-06-23 | — | 低频维护 |
| Dadoum/anisette-v3-server | 2026-09-23 | — | 非常活跃 |
| SignTools/SignTools | 2026-09-21 | AGPL-3.0 | 活跃 |
| nab138/iloader | 2026-09-23 | MIT | 非常活跃 |
| sickcodes/Docker-OSX | 2025-11-11 | GPL-3.0 | 活跃 |

---

## 3. 签名流程的关键技术点

### 3.1 p12 证书 + mobileprovision 的生成流程（模拟 Xcode 与 Apple 服务器交互）

以 SideSign 源码为最新一手依据（AltStore 旧栈同样走这套接口）：

1. **认证（GSA / GrandSlam）**：向 `https://gsa.apple.com/grandslam/GsService2` 发起 SRP-6a 加密握手，身份为 `com.apple.gs.xcode.auth`（即伪装成 Xcode）；双重认证走 `https://idmsa.apple.com/appleauth/auth/devices` 等接口；所有请求必须带 anisette 头。
   来源：https://github.com/SideStore/SideSign/blob/main/Sources/Constants.swift
2. **调开发者服务 API**：以 Xcode 的 clientId `XABBG36SBA`、协议版本 `QH65B2` 访问 `https://developerservices2.apple.com/services/QH65B2/*.action`（plist RPC），依次完成：`listTeams`（确认 Personal Team）→ `viewDeveloper` → 注册设备 UDID → 注册 App ID → 提交 CSR 创建 iOS Development 证书 → `ios/downloadTeamProvisioningProfile.action` 下载 Team Provisioning Profile（对免费/付费账号均适用的 ephemeral profile，每次调用即时生成）。
   来源：https://github.com/SideStore/SideSign/blob/main/Sources/Constants.swift 、https://github.com/SideStore/SideSign/blob/main/specs/profile_endpoints_spec.py 、https://github.com/altstoreio/AltStore/blob/marketplace/AltPlugin/ALTPluginService.m （listTeams 调用实例）
3. **本地配套产物**：2048 位 RSA 私钥 + PKCS#10 CSR 在本地生成；拿到证书后私钥+证书导出为 `.p12`（PKCS#12）供签名工具使用。
   来源：https://github.com/SideStore/SideSign/blob/main/README.md
4. 注：fastlane 生态里的 "olympus"（`https://appstoreconnect.apple.com/olympus/v1/session`）是 App Store Connect 网页会话接口，属于另一条（付费账号管理向）认证路线，与上面的 Xcode 开发者服务接口不是同一套，注意区分。
   来源：https://github.com/fastlane/fastlane/blob/master/spaceship/lib/spaceship/client.rb

### 3.2 entitlements 修改

- 黄金法则：**签名后应用的 entitlements 必须是 profile 里 Entitlements 字典的子集**。zsign 的默认行为就是不传 `-e` 时直接把 mobileprovision 里的 `Entitlements` 原样取出写入签名，并从中读取 `application-identifier`、`TeamIdentifier` 和开发证书。
  来源：https://github.com/zhlynn/zsign/blob/master/src/openssl.cpp （ZSignAsset::Init，约 844–931 行）
- 免费账号的 profile 只含最基础 entitlements，所以重签时要**删掉原应用里 push/iCloud/app groups 等 entitlement**（否则校验失败或功能异常），这对应 zsign 的 `-e` 自定义 entitlements 和 SignTools 的 "configurable signing including entitlements"。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md 、https://github.com/SignTools/SignTools/blob/master/README.md
- 细节：zsign 源码按 `get-task-allow` 是否为 true 决定是否设置 `CS_EXECSEG_ALLOW_UNSIGNED` 执行段标志，与 Apple codesign 行为对齐。
  来源：https://github.com/zhlynn/zsign/blob/master/src/archo.cpp （约 389–400 行注释）
- SideSign 则会在签名时动态校验并重写 `application-identifier`、`team-identifier`、`keychain-access-groups`。
  来源：https://github.com/SideStore/SideSign/blob/main/README.md

### 3.3 bundle id 改写

- 每个 App ID 名额宝贵（10 个/周），所以把多个不同 bundle id 的 IPA 改成同一个已注册 App ID 对应的 bundle id 是常规省名额手段：zsign `-b com.new.bundle.id` 改 Info.plist 里的 `CFBundleIdentifier`。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md
- 匹配规则：最终 bundle id 必须与 profile 的 `application-identifier` 匹配（explicit profile 为 `TEAM_ID.xxx` 精确匹配；wildcard `TEAM_ID.*` 可匹配任意后缀但 entitlements 受限，见 2.4）。
  来源：https://github.com/SignTools/SignTools/blob/master/FAQ.md
- 注意应用内的 app group id、`keychain-access-groups` 等也引用旧 bundle id，需要同步改写（SideSign 自动处理 keychain-access-groups；用 zsign 时需配合 `-e`）。
  来源：https://github.com/SideStore/SideSign/blob/main/README.md

### 3.4 dylib 注入的兼容性问题

- 机制：往主可执行 Mach-O 里加 `LC_LOAD_DYLIB` / `LC_LOAD_WEAK_DYLIB` load command（zsign `-l` / `-w`），dylib 拷到 .app 根部，用 `@executable_path/xxx.dylib` 相对路径引用；注入的 dylib 会被一并重签。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md 、https://github.com/zhlynn/zsign/blob/master/src/bundle.cpp （约 836–846 行）
- 关键坑（README 一手确认）：**app extension（PlugIns/*.appex）是独立进程，不继承主应用的注入 dylib**——需要 `-P` 参数往每个扩展里也注入（通过 `@executable_path/../../xxx.dylib` 相对引用根部的同一份 dylib）。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md
- 其他兼容性手段（zsign 内置）：删除扩展 `-E`、删除 watch app `-W`、移除 `UISupportedDevices` `-U`、改 MinimumOSVersion `-M`——都是为解决"重签后装不上/跑不起"的常见问题。
  来源：https://github.com/zhlynn/zsign/blob/master/README.md
- 若 tweak 依赖调试能力（如模拟器/越狱类应用），需 profile 带 `get-task-allow=true`（Development 证书才有）。
  来源：https://github.com/SignTools/SignTools/blob/master/FAQ.md

---

## 4. 对本项目（Linux 环境 IPA 下载+签名+安装）的落地建议

1. **签名核心用 zsign**（MIT、活跃、Linux 原生、功能覆盖 entitlements/bundle id/dylib 注入），避免自己实现 CMS 签名。
2. **证书与 profile 的获取**两条路：
   - 简单路：复用 AltServer-Linux / iloader 这类现成实现（它们已封装 GSA 登录 + developerservices2 API + anisette），把产出的 p12 + mobileprovision 交给 zsign。
   - 自研路：参考 SideSign 的 Constants.swift / specs 自己实现 GSA + developerservices2 调用（Swift 写但接口文档齐全）。
3. **anisette 自建**：docker 跑 `dadoum/anisette-v3-server`，避免公共服务器锁号风险；并建议用户用小号 Apple ID。
4. **安装通道**：Linux 上用 libimobiledevice 系工具（zsign `-i` 即调 ideviceinstaller；无线安装可用 netmuxd 方案）。
5. **产品层面必须暴露的限制**：7 天续签（做定时任务/提醒）、3 应用上限、10 App ID/周、部分功能（push/iCloud/内购）重签后失效。

## 5. 澄清：7 天过期的适用范围（2026-09-24 补充，回应用户反馈）

"7 天过期"仅适用于**免费开发者账号重签名**这一类型。用户反馈"自己签名安装从未遇到 7 天过期"，最可能的原因是实际使用的并非免费 ID 重签，而是：付费开发者账号（约 1 年）、原版 App Store 签名安装（不过期，Asspp 模式）、企业证书，或 AltStore/SideStore 的后台自动续签。完整的签名类型有效期对照表与两条产物路径（原版安装 vs 重签安装）见 [04-installation-modes.md](./04-installation-modes.md) §2。本项目应把"是否重签"作为用户显式选择，而非默认必做步骤。
