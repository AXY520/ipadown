# 安装方式：有线 / 无线 / 在线（OTA）——补充调研

一句话总结：**有线走 go-ios（iOS 17+ 需隧道），在线安装走 Apple 官方 `itms-services` OTA 机制（HTTPS + manifest.plist），两者可以共用同一套「产物签名」前置流程；而"7 天过期"只存在于重签名路径，原版 App Store 签名安装不过期。**

## 1. 三种安装通道

### 1.1 有线（USB）——详见 03 篇
- 主依赖 `danielpaulus/go-ios`：`ios install --path=app.ipa`；iOS 17+ 先 `ios tunnel start` 建立 RSD 隧道（来源：https://github.com/danielpaulus/go-ios ）。
- 兜底 `ideviceinstaller`（iOS ≤16 稳定，iOS 17+ 不可靠）（来源：https://github.com/libimobiledevice/ideviceinstaller ）。

### 1.2 无线局域网配对（同 WiFi，可选增强）
- iOS 设备与主机配对过一次并开启「通过 Wi-Fi 与此 iPhone 同步」后，usbmuxd 生态可直接发现网络设备，go-ios 也支持 `--address/--rsd-port` 连接网络侧 RSD 隧道（来源：https://github.com/danielpaulus/go-ios README 命令表 `--address=<ipv6addr>`、`--rsd-port=<port>`）。
- 价值：免插线，但仍需设备在线且已配对；作为有线的补充而非主路径。

### 1.3 在线安装（OTA / itms-services）——本篇重点
Apple 官方支持「网页无线安装」机制（原企业分发通道），流程为：网页链接 → `itms-services://?action=download-manifest&url=https://<host>/manifest.plist` → 系统下载 manifest → 拉取 IPA 安装。官方硬性要求（来源：Apple Platform Deployment 官方文档「Distribute proprietary in-house apps to Apple devices」https://support.apple.com/en-gb/guide/deployment/depce7cefc4d/web ，2026-09-24 实测可访问）：
1. IPA 格式 + 相应的 provisioning profile；
2. **XML manifest 文件**（plist，含 `software-package` 的 IPA URL、图标 URL、bundle-identifier、bundle-version、title；完整键值见 Apple Developer 的 Install Application command 文档）；
3. **必须 HTTPS**，且证书被设备信任；
4. 设备能访问苹果服务器完成校验；
5. **iOS/iPadOS 18+ 注意**：手动（非 MDM）安装的企业/自签 App **需要重启一次设备**才能完成对 provisioning profile 的信任（官方原文确认）。

### 1.4 OTA 在本项目的工程化设计
- **局域网 OTA**：Mac/Linux 主机起 HTTPS 服务，手机同一 WiFi 打开链接/扫码安装。难点是"受信任的 HTTPS 证书"：
  - 方案 A（推荐，抄 Asspp 作业）：拥有一个域名，给 `*.ipa.你的域名.com` 签免费通配符证书（Let's Encrypt DNS-01），DNS 记录指到局域网 IP（如 192.168.x.x）。Asspp 正是用 `app.localhost.qaq.wiki` 的受信证书在本机起 Vapor HTTPS 服务完成自安装的（来源：https://github.com/Lakr233/Asspp/blob/main/Asspp/Backend/Installer/Installer.swift 、Installer+TLS.swift ）；同类公共服务还有 localhost.direct（Asspp README 致谢列表）。
  - 方案 B：自签 CA，但需用户先在设备上安装并信任 CA 描述文件，体验差，仅作兜底。
- **manifest 生成**：按官方字段模板生成即可，无加密要求；IPA/图标/manifest 三个静态文件 + 一个 itms-services 链接页。
- **iOS 18 重启提示**必须在 UX 里给出，否则用户会以为安装失败。

## 2. 为什么你自己签的包"没有遇到过 7 天过期"

7 天过期**只针对"免费开发者账号（free provisioning）重签名"**这一种签名类型（Apple 官方限制，见 02 篇 §1）。你的实际情况大概率是以下之一：

| 签名类型 | 有效期 | 典型场景 |
|---|---|---|
| **原版 App Store 签名（不重签）** | **不过期**（Apple 的 FairPlay 签名，跟随账号 license） | Asspp 本机安装、Apple Configurator 导出安装——IPA 原封不动装到已拥有该 App 的账号所在设备 |
| 付费开发者账号（$99/年） | 证书/profile 约 1 年 | 开发者自签 |
| 企业证书（Enterprise Program） | 约 1-3 年 | 蒲公英/fir.im 类分发（滥用会被苹果吊销） |
| AltStore/SideStore（免费 ID） | 7 天，但**后台自动续签** | 用户无感知，实际一直在续 |
| 免费开发者账号手动重签 | 7 天，过期打不开 | ipasign 类工具 |

结论：本项目应把安装分成两条产物路径，并把"是否重签"作为用户的显式选择：
- **路径 A（原版安装，不过期）**：ipatool/ApplePackage 下载的 IPA 不改任何字节，通过 OTA（或有线 installd 对商店包的校验路径）装到设备；前提是**设备上的 App Store 账号拥有该 App**（免费 App 在设备 App Store 点一次"获取"即可）。这正是 Asspp 的工作方式（来源：https://github.com/Lakr233/Asspp 中文 README「直接从 Apple 服务器下载官方正版 IPA」+ Installer 实现）。
- **路径 B（重签名安装）**：zsign 重签后安装，用于改 bundle id/多开/注入等场景，需接受签名类型的有效期约束（免费 7 天 + 3 应用上限；付费 1 年）。

## 3. 对本项目的落地建议

1. 安装模块设计为**双通道**：`install --usb`（go-ios）与 `install --ota`（局域网 HTTPS + itms-services），产物与签名路径正交（A/B 都能走任一通道）。
2. OTA 需要一次性基础设施投入：一个域名 + 通配符证书 + 动态 DNS 记录更新（指到主机局域网 IP）；做完后用户侧体验是"手机扫码即装"。
3. 原版路径（A）要检测并提示设备 App Store 账号是否已拥有该 App；重签路径（B）要提示有效期与 iOS 18 重启要求。
4. 无线配对通道（1.2）作为 v2 增强，不进 MVP。
