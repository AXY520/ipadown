# 01 · IPA 文件获取/下载方案调研

**一句话总结：在 Linux 上直接获取官方 IPA 的现实路径基本只有 [`majd/ipatool`](https://github.com/majd/ipatool)（Go 编写的 CLI，原生支持 Linux，自带 Apple ID 登录/2FA、免费 App 自动"购买"取 license、指定版本下载）；`ipatool-py` 仅在下历史版本这一细分场景有补充价值；Apple Configurator 抓包方案仅限 macOS；第三方 IPA 分享站有法律与安全风险，不建议作为产品方案。**

> 调研时间：2026-09-24。以下结论均以一手来源（GitHub 官方仓库 README / 源码 / GitHub API）为准，关键结论后附来源 URL。

---

## 1. majd/ipatool（重点方案）

### 1.1 项目概况与活跃度

- 仓库：<https://github.com/majd/ipatool>，描述为「在 App Store 搜索 iOS/iPadOS/tvOS/visionOS/macOS 应用并下载 .ipa 或 macOS .pkg 的命令行工具」（来源：<https://api.github.com/repos/majd/ipatool>）。
- 开源协议：**MIT**（来源：<https://github.com/majd/ipatool/blob/main/LICENSE>）。
- 活跃度：约 11.3k star；最近一次 commit 2026-09-19；最新 release **v2.6.0（2026-09-13）**，2026 年内已发布 v2.3.0→v2.6.0 多个版本，项目处于活跃维护状态（来源：<https://api.github.com/repos/majd/ipatool/releases>）。
- 成熟度：项目 2021-05-22 首发 v1.0.0（Swift 实现），2022-12-08 发布 v2.0.0（Go 重写），至今 4 年以上历史、30+ 个 release，配套单元测试完善（`pkg/appstore/*_test.go` 覆盖各 API 路径）（来源：<https://api.github.com/repos/majd/ipatool/releases?per_page=100>、<https://github.com/majd/ipatool/tree/main/pkg/appstore>）。

### 1.2 平台支持（含 Linux）

- README 明确要求：「支持的操作系统（macOS、Linux、Windows 或 iOS）+ 一个已配置好使用 App Store 的 Apple 账户」（来源：<https://github.com/majd/ipatool/blob/main/README.md>）。
- **Linux 是一等公民**：GitHub Releases 直接提供 `ipatool-2.6.0-linux-amd64.tar.gz` / `linux-arm64.tar.gz` 预编译二进制，另有 Windows、macOS、越狱 iOS 构建（来源：<https://github.com/majd/ipatool/releases/tag/v2.6.0>）。
- 也可 `go build` 自行编译（README "Compiling" 一节）。

### 1.3 功能清单（v2.6.0）

子命令：`auth login/info/revoke`（认证）、`search`（搜索）、`download`（下载）、`purchase`（获取免费 App license）、`list-purchases`（列出已购 App）、`list-versions`（列出可用版本）、`get-version-metadata`（指定版本元数据）（来源：README Usage 一节，<https://github.com/majd/ipatool/blob/main/README.md>）。

- `download` 支持 `--external-version-id` 指定版本（默认最新版）、`--platform`（iphone/ipad/appletv/visionos/macos）、`--purchase`（缺 license 时自动获取）（来源：<https://github.com/majd/ipatool/blob/main/cmd/download.go>）。
- 下载产物是**带 Apple FairPlay DRM 的官方 IPA**（与 App Store 分发的一致），下载后工具会把 sinf 复制回 IPA 包内保证包完整（`ReplicateSinf`，来源：<https://github.com/majd/ipatool/blob/main/cmd/download.go>）。v2.6.0 起还会内嵌 iTunes artwork 并改善 OTA 安装兼容性（来源：<https://github.com/majd/ipatool/releases/tag/v2.6.0>）。

### 1.4 Apple ID 登录与 2FA 流程

- 交互模式：`ipatool auth login` 依次提示 `enter email:` → 密码（掩码输入）→ 若账户开了双重认证则提示 `enter 2FA code:`（来源：<https://github.com/majd/ipatool/blob/main/cmd/auth.go>）。
- 非交互/自动化：`--non-interactive` 模式下必须用 `--email` / `--password` / `--auth-code` flag 显式传参，缺 2FA 码时会报错「2FA code is required; run the command again and supply a code using the `--auth-code` flag」（来源：<https://github.com/majd/ipatool/blob/main/cmd/auth.go>）。
- 实现细节：登录走 Apple 私有 SAP 认证端点（从 Apple 的 "bag" 配置动态获取，不允许回退到非官方端点），密码与 6 位 2FA 码拼接后提交（`appstore_login.go` 中 `"password": fmt.Sprintf("%s%s", password, authCode)`）；2FA 码必须恰好 6 位数字，否则会报「2FA code must contain exactly six digits」（来源：<https://github.com/majd/ipatool/blob/main/pkg/appstore/appstore_login.go>）。
- 会话复用：登录成功后凭证（passwordToken 等）存入系统 keychain，cookie jar 存入状态目录（支持 `XDG_STATE_HOME`/`XDG_DATA_HOME`，v2.6.0 新增）；`auth info` 查看当前账户、`auth revoke` 吊销凭证（来源：<https://github.com/majd/ipatool/blob/main/cmd/auth.go>、<https://github.com/majd/ipatool/releases/tag/v2.6.0>）。

### 1.5 下载限制（license / 区域 / 付费）

- **license 是硬前提**：下载请求若返回 `FailureTypeLicenseNotFound`，ipatool 报 `ErrLicenseRequired`（"license is required"）。即该 Apple ID 必须已"拥有"（购买/免费获取过）该 App（来源：<https://github.com/majd/ipatool/blob/main/pkg/appstore/appstore_download.go>）。
- 历史上（v1.x Swift 版）这只能去 iOS 设备/App Store 上手动点"获取"，当时的报错原文是「Your Apple ID does not have a license for this app. Download the app on an iOS device to obtain a license.」（来源：<https://github.com/majd/ipatool/blob/v1.0.9/Sources/CLI/Commands/Download.swift>）。
- **v2 已内置解决办法**：`purchase` 子命令（或 `download --purchase`）直接调用 `buyProduct`（`appExtVrsId=0`、`price=0`、`productType=C`）为**免费 App** 取得 license，无需再摸出 iOS 设备（来源：<https://github.com/majd/ipatool/blob/main/pkg/appstore/appstore_purchase.go>）。
- **付费 App 不支持代买**：源码明确 `if input.App.Price > 0 { return errors.New("purchasing paid apps is not supported") }`——付费 App 必须事先在设备/App Store 里用该账号买过（来源：同上 `appstore_purchase.go`）。
- **区域限制**：搜索的 country code 由登录账户的 StoreFront 推导（`countryCodeFromStoreFront(input.Account.StoreFront)`），请求头带 `X-Apple-Store-Front`。即只能搜/下 **Apple ID 所属国家/地区 storefront 上架的 App**；下载某区独有 App 需要对应区的账号（来源：<https://github.com/majd/ipatool/blob/main/pkg/appstore/appstore_search.go>、<https://github.com/majd/ipatool/blob/main/pkg/appstore/appstore_purchase.go>）。
- README 的 Requirements 还隐含一条：Apple 账户需「already configured to use the App Store」（即该账号已在 App Store 激活/同意过条款，全新未用过 App Store 的账号可能失败）（来源：<https://github.com/majd/ipatool/blob/main/README.md>）。

### 1.6 关于 "licenseVersionFlag"

- 调研中**在全部一手源码内均未发现 `licenseVersionFlag` 这个字段**：majd/ipatool 当前 main 分支（Go）、v1.0.9 标签（Swift）、NyaMisty/ipatool-py、wf021325/ipatool.js、mineek/MuffinStore 全部 grep 为零；GitHub 全站 code search 与 issue search 也是 0 命中（来源：`gh api "search/code?q=licenseVersionFlag"` 返回 `total_count: 0`）。
- 与这个概念最接近的真实机制是上文 1.5 的 **license（购买凭证）+ 版本号（externalVersionId / appExtVrsId）** 两条线：取 license 用 `buyProduct`，选版本用下载请求里的 `externalVersionId`（iOS 端历史版本 ID 即 `appExtVrsId`，见 ipatool-py：<https://github.com/NyaMisty/ipatool-py/blob/master/reqs/store.py>）。如果任务上游资料里出现 "licenseVersionFlag"，大概率是对「下载需先持 license」这一机制的二手转述，建议以本文 1.5 为准。

### 1.7 Keychain / passphrase 问题（Linux 部署要点）

- ipatool 用 keyring 抽象（`github.com/byteness/keyring`），允许的后端按优先级为：macOS Keychain → **Secret Service（gnome-keyring / KWallet，走 D-Bus）** → **加密文件后端**（来源：<https://github.com/majd/ipatool/blob/main/cmd/common.go>）。
- Linux 桌面环境：有 Secret Service 时直接工作。**无头服务器（无 D-Bus keyring）会落到文件后端**，此时必须提供解锁口令：交互模式下会提示 `enter passphrase to unlock ... (this is separate from your Apple ID password)`；非交互模式必须传 `--keychain-passphrase`，否则报错 "keychain passphrase is required when not running in interactive mode"（来源：同上 `cmd/common.go`，及 README 的 `--keychain-passphrase` flag 说明）。
- 工程提示：CI/自动化跑 ipatool 的最小配置 = `ipatool auth login --non-interactive -e <email> -p <password> --auth-code <2fa> --keychain-passphrase <任意口令>`；keychain 文件与 cookie 存在状态目录，可通过 `XDG_DATA_HOME`/`XDG_STATE_HOME` 固定位置以便持久化（来源：<https://github.com/majd/ipatool/releases/tag/v2.6.0>、<https://github.com/majd/ipatool/blob/main/cmd/state_directory.go>）。

---

## 2. NyaMisty/ipatool-py（备选，主打"历史版本"）

- 仓库：<https://github.com/NyaMisty/ipatool-py>。约 730 star；**无 LICENSE 文件**（GitHub API `license: null`，法律上属于保留所有权利，商用/再分发需谨慎）（来源：<https://api.github.com/repos/NyaMisty/ipatool-py>）。
- 活跃度：最近 commit 2025-03-28（修迭代 bug），再往前 2024-09-01（新增 `volumeStoreDownloadProduct` 支持旧版本下载）；更新稀疏，属"能用但维护低频"（来源：<https://api.github.com/repos/NyaMisty/ipatool-py/commits>）。
- 功能：三个命令 `lookup`（查信息/appVerId）、`historyver`（列出某 App 全部历史 appVerId）、`download`（下载，支持 `--purchase` 顺带取 license、`-i` appID 或 `-b` bundleID、`-c` 国家代码如 JP）（来源：<https://github.com/NyaMisty/ipatool-py/blob/master/README.md>）。
- **差异化价值 = 批量下历史版本**：`historyver` 拿到全部 appVerId 后可 `--downloadAllVersion` 批量拉取（来源：同上 README）。注意 majd/ipatool v2.4+ 也已有 `list-versions` + `--external-version-id`，此优势在收窄。
- **主要门槛**：下历史版本的 `buyProduct` 需要 Apple 的 kbsync 签名，必须外搭一个 "iTunes Server" 提供签名头，三条路：(a) GitHub Action `action-ipadown`（README 明确**不支持 2FA**）；(b) Windows 上装 iTunes 12.6.5.3 + frida 跑 `actions-iTunes-header` 脚本（支持 2FA）；(c) 越狱 iOS 设备跑 KbsyncTool（支持 2FA）（来源：同上 README）。
- 认证实现较旧：直接 POST `MZFinance.woa/wa/authenticate`（伪装 `Configurator` UA 的 Apple ID+密码登录），代码内没有内建的 2FA 交互流程（来源：<https://github.com/NyaMisty/ipatool-py/blob/master/reqs/store.py>）。社区反馈还可能要求先在 iTunes 里"授权电脑"并勾过"不再询问密码"（README 引用 issue #26）。
- 结论：**不建议做主链路**（维护低频、无协议、依赖外挂签名服务器）；仅在「必须批量下某个 App 的全部历史版本」时作为专项工具参考。

## 3. Apple Configurator 抓包/缓存方案

- 原理：Apple 官方的 Apple Configurator（Mac App Store，id1037126344，当前版本 2.20、要求 macOS 15.7+）在「登录 Apple ID → 给连接的设备添加 App」的过程中会先从 App Store 下载完整 IPA 到 Mac 本地缓存目录，再推送给设备；在安装完成前从缓存目录把 IPA 拷出即得官方包（该缓存路径属社区文档化的技巧，macOS 下经典位置为 `~/Library/Group Containers/K36BKF7T3D.group.com.apple.configurator/...`）。Apple 官方入口：<https://support.apple.com/apple-configurator>（HTTP 200），应用信息来源：<https://itunes.apple.com/lookup?id=1037126344>。
- 该思路的旁证：ipatool-py 的下载请求就是伪装 Apple Configurator 的 User-Agent（`Configurator/2.15 ...`）调同一批私有商店接口，说明这套"Configurator 通道"是苹果官方客户端自己也在用的（来源：<https://github.com/NyaMisty/ipatool-py/blob/master/reqs/store.py>）。
- 对本项目的适用性：**仅限 macOS**（无 Linux/Windows 版），且需要一台 Mac + 目标 Apple ID 登录；得到的同样是带 FairPlay DRM 的官方 IPA。在 Linux 环境下不具备可操作性，仅作为 macOS 备选路径或理解 ipatool 原理的参考。

## 4. 第三方 IPA 分享站 / 仓库（仅列举，点到为止）

- 形态举例：`armconverter.com`（HTTP 200，在线"代下"）、`decrypt.day`（本次调研请求返回 HTTP 451「因法律原因不可用」，本身就是风险注脚）等（来源：本次调研 `curl` 实测，2026-09-24）。
- 风险概述（不建议采用，仅备查）：
  - 法律：绕过 Apple 官方渠道下载他人 App 的 IPA 违反 App Store 服务条款；「decrypted IPA」还涉及去除 FairPlay DRM 的破解行为，存在著作权侵权风险；站点随时可能被 DMCA/封禁（如上述 451）。
  - 安全：无法验证包未被篡改/植入恶意代码；「输入 Apple ID 帮你代下」类服务有凭证钓鱼风险。
  - 工程：版本滞后、可用性无保障，不构成可依赖的产品链路。

---

## 5. 参考项目：Lakr233/Asspp 与 ApplePackage（用户指定）

用户点名参考的项目，定位是「在 iPhone/iPad 上直接管理多区 Apple ID 并下载 IPA」，对本项目有很强的对照价值。

### 5.1 Asspp 本体

- 定位：多账号/多区 App Store 管理器，可在设备上直接从苹果服务器下载**官方签名 IPA**（非脱壳包），支持历史版本下载；iOS 17+ / macOS 15+，Swift 编写，MIT 协议，约 5.9k star，2026-07 仍在更新（来源：https://github.com/Lakr233/Asspp 、https://api.github.com/repos/Lakr233/Asspp）。
- 与 ipatool 的关系：README「Special Notice」明确声明「utilizes the same underlying communication protocol as ipatool」，即同一套 App Store 私有协议（登录 → 获取 license/buyProduct → StoreDownload 端点下载）；差异在于 Asspp 是 GUI + 多账号多区管理 + 设备端安装（来源：https://github.com/Lakr233/Asspp/blob/main/README.md）。
- 风险提示（README 原话，值得本项目照抄给用户）：该协议随时可能因苹果风控/网关调整失效且作者未必能修；**强烈建议不要用主力 Apple ID**（封号理论上可能导致激活锁）；设备 GUID 属敏感信息不可泄露（来源：同上 README）。
- 安装环节（macOS 侧）直接调用 Xcode 自带的 `devicectl`（`CoreDevice.framework` 私有框架），即 macOS 依赖 Xcode，无法在 Linux 复用（来源：https://github.com/Lakr233/Asspp/blob/main/Asspp/Backend/DeviceCTL/DeviceCTL.swift ）。这反向印证本项目安装链路应选 go-ios/pymobiledevice3（见 03 篇）。

### 5.2 核心库 ApplePackage（git submodule，独立仓库）

- 「ipatool rewrite in library and cli using Swift」：把 ipatool 的协议层抽成了 Swift Package（库 `ApplePackage` + CLI `ApplePackageTool`），MIT 协议，2026-08-29 仍有提交（来源：https://github.com/Lakr233/ApplePackage 、https://api.github.com/repos/Lakr233/ApplePackage 、https://github.com/Lakr233/Asspp/blob/main/.gitmodules ）。
- 源码结构与 ipatool 命令集一一对应：`Commands/Authenticate.swift`（账号密码 + 2FA code 登录）、`Purchase.swift`（取 license）、`Download.swift`、`Search.swift`，以及独立的 `VersionFinder.swift`/`VersionLookup.swift`——**历史版本查询是原生实现的**，不像 ipatool-py 需要外挂自建 iTunes Server 签名旧版本请求（来源：https://github.com/Lakr233/ApplePackage/tree/main/Sources/ApplePackage/Commands ）。
- CLI 用法与 ipatool 几乎同构：`login <email> <password> [--code]` → `search` → `versions <email> <bundle-id>` → `download <email> <bundle-id> [--version-id] --output`（来源：https://github.com/Lakr233/ApplePackage/blob/main/README.md ）。
- 对 Linux 的可移植性：Package.swift 仅声明 iOS 15 / macCatalyst 14 / macOS 11 平台，但依赖全是 server-side Swift 生态（AsyncHTTPClient、ZIPFoundation、swift-log、swift-argument-parser），理论上有移植到 Linux Swift 的可能，工作量未验证；短期本项目仍建议直接用 Go 版 ipatool 二进制（来源：https://github.com/Lakr233/ApplePackage/blob/main/Package.swift ）。

### 5.3 对本项目的借鉴点

1. **协议层直接抄作业**：ApplePackage 是目前最干净的一手 Swift 版协议实现（比 ipatool Go 源码更贴近 App 形态），做下载模块时值得对照阅读，尤其是 `VersionFinder`（历史版本）和 `StoreDownloadEndpoint`（下载端点鉴权）两个文件。
2. **产品形态参考**：Asspp 的「账号池（Keychain 持久化）+ 按区自动选账号 + 版本历史 UI」是本项目下载模块可借鉴的交互/数据模型（`AppStore.swift` 中 `UserAccount` 持久化 + `rotate` 会话刷新思路）。
3. **风控共识**：两个项目都强调用小号 Apple ID；本项目在 UX 上应把「不要用主力号」做成醒目提示。


## 6. 方案对比速查

| 维度 | majd/ipatool | Lakr233/ApplePackage | NyaMisty/ipatool-py | Apple Configurator 缓存 | 第三方 IPA 站 |
|---|---|---|---|---|---|
| 协议 | MIT | MIT | 无 LICENSE（保留权利） | Apple 专有软件 | — |
| 活跃度 | 活跃（2026-09 仍发版） | 活跃（2026-08 仍提交） | 低频（2025-03 最后 commit） | Apple 官方持续更新 | 不稳定 |
| Linux 可用 | ✅ 原生二进制 | ⚠️ 未声明（依赖可移植） | ✅ 但需外挂 iTunes Server | ❌ 仅 macOS | 视站点 |
| 2FA | ✅ 内建交互/`--auth-code` | ✅ `--code` 参数 | ⚠️ 主程序无内建 2FA 流程 | ✅ 走官方客户端 | — |
| 需已购 license | ✅（免费 App 可 CLI 自取） | ✅（Purchase 命令） | ✅（同） | ✅（同账号） | 不需要（但违法风险） |
| 付费 App | ❌ 必须事先买 | ❌ 同 | ❌ 同 | ❌ 同 | — |
| 历史版本 | ✅ `--external-version-id` + `list-versions` | ✅ 原生 VersionFinder | ✅ 强项（批量） | ❌ | 部分 |
| 形态 | CLI + Go 库 | Swift 库 + CLI + Asspp GUI | CLI | 官方 App | — |
| 推荐度 | ★★★★★ 主方案 | ★★★★ 参考实现 | ★★☆ 专项补充 | ★★☆ macOS 备选 | ☆ 不建议 |

## 7. 对本项目（Linux 环境）的建议

1. 主链路用 `majd/ipatool`：Linux 二进制 + MIT + 活跃维护，`auth login`（含 2FA）→ `search` →（免费 App 自动 `purchase`）→ `download`，全流程可脚本化（`--non-interactive` + `--keychain-passphrase`）。
2. 账号侧约束要提前告知用户：Apple ID 需已在 App Store 激活；免费 App 可直接获取，付费 App 必须先买；只能访问账号所属区的 storefront。
3. 若产品有"历史版本下载"需求：优先用 ipatool 自带的 `list-versions`/`--external-version-id`；实现细节可对照 Lakr233/ApplePackage 的 `VersionFinder`（最干净的原生实现，见 §5）；ipatool-py 仅作为批量历史版本的对照参考。
4. 用户指定的参考项目 Asspp（§5）其价值在协议层实现与产品形态（多账号池/按区切号/版本历史 UI），其 macOS 安装链路依赖 Xcode `devicectl` 不可复用；并把「不要用主力 Apple ID」的风控提示纳入产品 UX。
5. 下载产物为官方 DRM 加密 IPA，后续"签名 + 安装到 iPhone"环节需要按此前提设计（与自签名重打包的输入不同），与后续调研主题（签名/安装）对接时需注意。
