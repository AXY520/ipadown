# 03 · Linux 上把 IPA 安装到 USB 连接的 iPhone：工具链调研

> **一句话总结**：iOS ≤16 走 `libimobiledevice`（`ideviceinstaller`）经典路线即可；iOS 17 起苹果把开发者服务迁到 CoreDevice/RemoteXPC，需要"隧道（tunnel）"，目前 Linux 上最省心、维护最活跃的方案是 `danielpaulus/go-ios`（单静态二进制 + `ios tunnel start`）或 `doronz88/pymobiledevice3`（pip 安装 + 免 root 用户态隧道），而 `ideviceinstaller` 虽仍能装 iOS 17 的包（iOS 18 已有失败报告），但已无法挂 DDI/调试，不建议作为新项目的唯一依赖。

调研时间：2026-09-24。以 GitHub 官方仓库 README / 源码 / 官方文档 / 官方 issue 为准。

---

## 1. 背景：iOS 17 的协议变化（CoreDevice / RemotePairing / Tunnel）

- 从 iOS 17.0 开始，苹果把**开发者服务**（调试、instruments、进程控制、挂 Developer Disk Image 等）从传统的 lockdownd/usbmuxd 通道迁移到 **CoreDevice / RemoteXPC** 体系，访问这些服务需要先和设备建立一条 **RSD（Remote Service Discovery）隧道**。
  来源：https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/
- 配对层面出现了 **RemotePairing**：iOS 17+ 的隧道配对走基于 IPv6 的远程配对链路；USB 连接时可通过 lockdownd 的 `CoreDeviceProxy` 服务直接拉起隧道（iOS 17.4+），17.0–17.3.1 则需要 Wi-Fi RemotePairing 或内核隧道。
  来源：https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/
- libimobiledevice 维护者 nikias 在官方 issue 中确认正在做 remoted（iOS 17 新通道）支持但"没有想象中简单"，且直言 "gestalt access is dead"；该支持**至今未进入任何发布版**。
  来源：https://github.com/libimobiledevice/libimobiledevice/issues/1638
- 受影响的旧工具：`ideviceimagemounter`、`idevicedebug`、`idevicescreenshot` 等在 iOS 17 上失效（见官方 issue #1452、#1547、#1465、#1490）。
  来源：https://github.com/libimobiledevice/libimobiledevice/issues/1452 ，https://github.com/libimobiledevice/libimobiledevice/issues/1490
- 关键细节：**纯安装 IPA** 走的 `installation_proxy`/`streaming_zip_conduit` 服务在 iOS 17 上仍可通过旧通道工作——ideviceinstaller 官方 issue #167 中用户确认 "For iOS 16, 17 it installs normally"（但同一 issue 报告 **iOS 18 安装报错**，至今 open）。iOS 17+ 上安装类操作官方推荐路径也是走隧道（go-ios/pymobiledevice3 均如此实现）。
  来源：https://github.com/libimobiledevice/ideviceinstaller/issues/167

---

## 2. libimobiledevice 生态

### 2.1 组件与角色

- **usbmuxd**：USB 复用守护进程，Linux 上是所有工具（包括 go-ios、pymobiledevice3，见 §5）与设备通信的地基；支持 udev/systemd 自动拉起。最新发布 1.1.1（2020-06），代码仍在维护。
  来源：https://github.com/libimobiledevice/usbmuxd ，https://github.com/libimobiledevice/usbmuxd/releases
- **libimobiledevice**：核心 C 库 + 命令行工具集（`idevicepair`、`ideviceinfo`、`ideviceimagemounter`、`idevice_id`、`idevicedate` 等）。2025-10-10 发布 1.4.0，是 1.3.0（2020-06）之后时隔 5 年的首个发布；仓库 8.2k star，最近 push 2026-06，仍在维护但节奏慢。
  来源：https://github.com/libimobiledevice/libimobiledevice ，https://github.com/libimobiledevice/libimobiledevice/releases
- **idevicepair**：管理配对（`pair`/`validate`/`unpair`），生成 host 与设备的信任记录（pair record）；配对本身在 iOS 17/18 上仍通过 usbmuxd 通道工作。
  来源：https://github.com/libimobiledevice/libimobiledevice （tools 目录），https://libimobiledevice.org/
- **ideviceinstaller**：安装/升级/卸载/枚举 App（`ideviceinstaller install app.ipa`）。最新发布 1.2.0（2025-10-30）。
  来源：https://github.com/libimobiledevice/ideviceinstaller ，https://github.com/libimobiledevice/ideviceinstaller/releases
- **ideviceimagemounter**：挂载 Developer Disk Image（DDI，`DeveloperDiskImage.dmg` + signature），是 iOS ≤16 上启用 debugserver/JIT 调试的前提；**iOS 17+ 失效**（见 §1）。
  来源：https://github.com/libimobiledevice/libimobiledevice/issues/1547

### 2.2 iOS 17+ 现状

- 可用：`idevicepair`（配对）、`ideviceinfo`（部分值——MobileGestalt 大量键已不可读）、`ideviceinstaller install`（iOS 17 实测可用，iOS 18 有 open 的报错 issue）。
  来源：https://github.com/libimobiledevice/libimobiledevice/issues/1638 ，https://github.com/libimobiledevice/ideviceinstaller/issues/167
- 不可用：挂 DDI、debugserver、截图、instruments 等一切开发者服务——没有 CoreDevice/tunnel 实现。
  来源：https://github.com/libimobiledevice/libimobiledevice/issues/1452 ，https://github.com/libimobiledevice/libimobiledevice/issues/1490
- 结论：**ideviceinstaller 可作为 iOS ≤17 的兜底安装器，但不能作为新项目唯一依赖**（iOS 18 已翻车、无维护承诺、无 DDI 能力）。

---

## 3. danielpaulus/go-ios

- 定位：跨平台（Linux/Windows/macOS）的 iOS 自动化 CLI + Go 库，目标是"生产可用的开源方案"，HeadSpin、Sauce Labs 等公司在用；MIT 许可证。
  来源：https://github.com/danielpaulus/go-ios
- **活跃度**：约 2.3k star；最新 release v1.3.2（2026-08-11），调研当天（2026-09-24）仍有 push；CHANGELOG 记录 2026 年内持续修隧道、安装、配对相关 bug。
  来源：https://github.com/danielpaulus/go-ios/releases ，https://github.com/danielpaulus/go-ios/blob/main/CHANGELOG.md
- **安装 IPA**：`ios install --path=/path/to/app.ipa`（也支持解压后的 .app 目录）；iOS 17+ 上通过隧道连 `streaming_zip_conduit` 服务完成安装。
  来源：https://github.com/danielpaulus/go-ios/blob/main/README.md ，https://github.com/danielpaulus/go-ios/blob/main/ios/zipconduit/zipconduit_installer.go
- **配对**：`ios pair` 可完成配对（supervised 设备甚至免手动点弹窗）；iOS 17+ 的隧道配对由 `tunnel start` 自动处理。
  来源：https://github.com/danielpaulus/go-ios/blob/main/README.md
- **iOS 17+ 支持方式**：`sudo ios tunnel start` 启动隧道守护进程（README 原话："For iOS 17+ devices you need to run `sudo ios tunnel start`"）。之后各命令通过本机 tunnel-info API（默认 127.0.0.1:28100）自动发现隧道并把连接路由到 RSD 地址，用户无需关心细节。
  - 内核 TUN 模式需要 root / CAP_NET_ADMIN；提供 `--userspace` 用户态隧道模式可**免 root**。
  来源：https://github.com/danielpaulus/go-ios/blob/main/README.md ，https://github.com/danielpaulus/go-ios/blob/main/cli_device_resolution.go ，https://github.com/danielpaulus/go-ios/blob/main/cmd_tunnel.go
- **DDI**：`ios image auto` 自动下载并挂载开发者镜像；iOS 17+ 使用"通用个性化（personalized）DDI"（从 deviceboxhq.com 拉取，经苹果 TSS `gs.apple.com` 签名个性化），支持离线缓存复用。
  来源：https://github.com/danielpaulus/go-ios/blob/main/ios/imagemounter/imagedownloader.go ，https://github.com/danielpaulus/go-ios/blob/main/ios/imagemounter/tss.go
- **其他与本项目相关的命令**：`ios apps`（列应用）、`ios info`（设备信息）、`ios devmode enable/get/reveal`（开发者模式）、`ios profile add/list/remove`（provisioning profile）、`ios launch`/`kill`/`debug`（启动/调试/JIT）、`ios runwda`（WebDriverAgent）、`ios sign app`（用 P12 + profile 重签 IPA）。
  来源：https://github.com/danielpaulus/go-ios/blob/main/README.md

---

## 4. doronz88/pymobiledevice3

- 定位：纯 Python 3 实现（要求 Python ≥3.9），同时是 CLI 和 Python API，跨 Windows/Linux/macOS；GPLv3 许可证。
  来源：https://github.com/doronz88/pymobiledevice3 ，https://pypi.org/pypi/pymobiledevice3/
- **活跃度**：约 2.8k star；发布极其频繁——PyPI 上 11.17.0（2026-09-22）到 11.19.0（2026-09-24，调研当天）连发 5 版；仓库当天有 push。三大工具中迭代最快。
  来源：https://pypi.org/pypi/pymobiledevice3/#history ，https://github.com/doronz88/pymobiledevice3
- **安装 IPA**：`pymobiledevice3 apps install <app.ipa>`（另有 `apps list/uninstall/query`）；连接选项统一支持 `--usbmux`（iOS ≤16 经典通道）、`--rsd` / `--tunnel` / `--userspace`（iOS 17+）。
  来源：https://doronz88.github.io/pymobiledevice3/cli/apps/
- **iOS 17+ 支持方式**（官方支持矩阵）：
  - Linux + iOS 17.4+：**默认自动建立免 root 的进程内用户态隧道**（纯 Python 网络栈 PyTCP，经 lockdownd 的 CoreDeviceProxy 服务），直接跑命令即可，无需预先启动任何守护进程；
  - Linux + iOS 17.0–17.3.1：需 `sudo pymobiledevice3 remote tunneld` 起一个特权内核隧道守护进程（默认绑 127.0.0.1:49151，带 HTTP API 和 websocket 桥接，可服务外部工具/多主机）。
  来源：https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/
- **DDI**：`pymobiledevice3 mounter auto-mount` 自动识别版本、下载并挂载 DDI；iOS ≤16 用经典 DDI，iOS 17+ 用 personalized DDI（走 TSS 个性化签名，或以 cryptex 形式经 cryptexd 安装），镜像来自配套的 `developer-disk-image` PyPI 包 / GitHub 仓库。
  来源：https://doronz88.github.io/pymobiledevice3/cli/mounter/ ，https://github.com/doronz88/pymobiledevice3/blob/master/pymobiledevice3/services/mobile_image_mounter.py ，https://github.com/doronz88/DeveloperDiskImage
- **provisioning profile**：`pymobiledevice3 provision install/list/remove/clear` 直接管理 .mobileprovision。
  来源：https://doronz88.github.io/pymobiledevice3/cli/provision/
- **其他能力**：`developer dvt`/`developer debugserver`（DTX/LLDB 调试、JIT）、`amfi`、`profile`、`syslog`、`pcap`、backup/restore 等，功能面是三者中最全的；官方文档站完整。
  来源：https://github.com/doronz88/pymobiledevice3 ，https://doronz88.github.io/pymobiledevice3/

---

## 5. Linux 上的共同地基：usbmuxd 仍然必不可少

- go-ios 默认通过 unix socket `/var/run/usbmuxd` 发现/连接设备（可用 `USBMUXD_SOCKET_ADDRESS` 覆盖）。
  来源：https://github.com/danielpaulus/go-ios/blob/main/ios/usbmuxconnection.go
- pymobiledevice3 同样使用 `/var/run/usbmuxd`（`USBMUXD_PIPE`），同样支持 `USBMUXD_SOCKET_ADDRESS`。
  来源：https://github.com/doronz88/pymobiledevice3/blob/master/pymobiledevice3/usbmux.py
- 结论：新项目在 Linux 上无论选哪个高层工具，**都需要用户机器上运行 usbmuxd**（各发行版仓库均有，见 §7）；iOS 17+ 的隧道是叠加在 usbmuxd 配对之上的第二层通道。

---

## 6. 安装流程整体步骤

### 6.1 iOS ≤16（经典 lockdown 路线）

1. **配对**：`idevicepair pair`（设备需解锁并点"信任此电脑"）。
   来源：https://libimobiledevice.org/
2. **查设备信息**：`ideviceinfo`（UDID、iOS 版本、设备型号），或 `ios info` / `pymobiledevice3 lockdown info`。
3. **挂 DDI（仅调试/JIT 需要）**：`ideviceimagemounter DeveloperDiskImage.dmg DeveloperDiskImage.dmg.signature`，或 `ios image auto` / `pymobiledevice3 mounter auto-mount`（自动下载匹配版本）。**单纯装 IPA 不需要挂 DDI。**
   来源：https://github.com/danielpaulus/go-ios/blob/main/README.md ，https://doronz88.github.io/pymobiledevice3/cli/mounter/
4. **安装**：`ideviceinstaller install app.ipa`，或 `ios install --path=app.ipa` / `pymobiledevice3 apps install app.ipa`。
   来源：https://github.com/libimobiledevice/ideviceinstaller
5. **provisioning profile 管理**：重装/刷新 `.mobileprovision` 用 `ios profile add` 或 `pymobiledevice3 provision install`；免费 Apple ID 签名 7 天过期，过期后需重签重装（属签名环节，见签名调研）。
   来源：https://doronz88.github.io/pymobiledevice3/cli/provision/

### 6.2 iOS 17+（CoreDevice / 隧道路线）

1. **配对**：USB 配对仍走 usbmuxd（`ios pair` / `pymobiledevice3 usbmux pair`）；go-ios 会在 `tunnel start` 时按需自动完成隧道侧配对。
   来源：https://github.com/danielpaulus/go-ios/blob/main/README.md
2. **建立隧道**（二选一）：
   - go-ios：`sudo ios tunnel start`（常驻守护；或 `--userspace` 免 root）；
   - pymobiledevice3：iOS 17.4+ 不需要任何预启动（命令内自动建用户态隧道）；17.0–17.3.1 需 `sudo pymobiledevice3 remote tunneld` 常驻。
   来源：https://github.com/danielpaulus/go-ios/blob/main/README.md ，https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/
3. **安装**：`ios install --path=app.ipa` 或 `pymobiledevice3 apps install app.ipa`（自动走隧道）。
   来源：https://github.com/danielpaulus/go-ios/blob/main/ios/zipconduit/zipconduit_installer.go ，https://doronz88.github.io/pymobiledevice3/cli/apps/
4. **挂 DDI（仅调试/JIT 需要）**：`ios image auto` 或 `pymobiledevice3 mounter auto-mount`（personalized DDI + TSS 签名）。
   来源：https://github.com/danielpaulus/go-ios/blob/main/ios/imagemounter/imagedownloader.go

### 6.3 开发者模式与 JIT 注意事项

- iOS 16+ 安装开发者签名的 App 后，必须在 **设置 → 隐私与安全性 → 开发者模式** 打开开关（开关开启需重启并在设备上确认），否则 App 无法启动；go-ios 提供 `ios devmode enable/get/reveal`（`--enable-post-restart` 可在重启后收尾）辅助该流程。
  来源：https://github.com/danielpaulus/go-ios/blob/main/main.go ，https://github.com/danielpaulus/go-ios/blob/main/cmd_device_management.go
- 需要 **JIT** 的 App（模拟器类、虚拟机等）必须先挂 DDI 并通过 debugserver 以 `get-task-allow`/调试方式启动：go-ios 用 `ios debug` / `ios launch`，pymobiledevice3 用 `developer debugserver`（可驱动外部 lldb）；iOS 17+ 这一切都必须建立在隧道之上，旧的 `idevicedebug` 已失效。
  来源：https://github.com/danielpaulus/go-ios/blob/main/README.md ，https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/ ，https://github.com/libimobiledevice/libimobiledevice/issues/1452
- 注意 go-ios `--userspace` / pymobiledevice3 用户态隧道的设备地址只在进程内有效，**外部调试器（lldb 等）连不到**，需要内核隧道（tunneld 或 go-ios 内核 TUN）或本地端口转发。
  来源：https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/

---

## 7. 分发考量：作为新项目依赖哪个最省心

| 工具 | Linux 安装方式 | 需要 root | 许可证 | 维护活跃度 | 作为依赖的评价 |
|---|---|---|---|---|---|
| usbmuxd（必备地基） | 发行版包（apt `usbmuxd`、Arch `usbmuxd`、Fedora 同） | 守护进程需 systemd/udev 系统级运行 | GPL（守护进程） | 稳定，发布慢但成熟 | 无论如何都要装；靠包管理器最稳 |
| libimobiledevice 工具集 | apt：`libimobiledevice-utils`、`ideviceinstaller`；Arch：官方仓库有 `libimobiledevice`，`ideviceinstaller` 仅 AUR；或源码 autogen 编译 | 否 | LGPL（库）/GPL（工具） | 中（2025 恢复发版：lib 1.4.0 / installer 1.2.0） | 发行版版本滞后；iOS 17+ 能力残缺，只适合 ≤17 兜底 |
| go-ios | GitHub Releases 单静态二进制（`go-ios-linux.zip`）；`npm i -g go-ios`；`go build`；可作 Go module 引入 | 仅内核隧道模式需要；`--userspace` 免 root | **MIT** | 高（2026 年内持续发版） | **最省心**：一个二进制搞定配对/隧道/DDI/安装/调试全链路，可直接随项目下发 |
| pymobiledevice3 | `pip install -U pymobiledevice3`（或 pipx）；仓库无官方 Dockerfile（tunneld 支持跑在容器里 + websocket 桥接） | 17.4+ 免 root；17.0–17.3.1 需 root tunneld | **GPLv3**（动态链接/打包分发需注意；作为外部 CLI 调用无碍） | 极高（几乎周更） | 功能最全、文档最好、有 Python API；代价是要带 Python ≥3.9 环境，GPLv3 不适合静态打包进闭源产物 |

来源：https://archlinux.org/packages/extra/x86_64/usbmuxd/ ，https://packages.debian.org/stable/usbmuxd ，https://github.com/danielpaulus/go-ios/releases ，https://www.npmjs.com/package/go-ios ，https://pypi.org/project/pymobiledevice3/ ，https://github.com/doronz88/pymobiledevice3

### 给本项目的建议

1. **主依赖选 go-ios**：下载一个静态二进制即可覆盖 Linux 用户的「配对 → 隧道 → 安装 → （可选）挂 DDI/JIT → profile 管理」全链路，MIT 许可证无分发顾虑，iOS 17+ 支持成熟。
2. **系统依赖 usbmuxd**：检测并提示用户用包管理器安装（或项目内提供一键脚本）。
3. **iOS ≤16 可兜底 ideviceinstaller**（若用户机器已有），但不是必需。
4. **pymobiledevice3 作为可选高级后端**：当需要 Python API 级别的精细控制（backup、高级调试、WebInspector）时再引入；注意 GPLv3 只以独立进程 CLI 方式调用。
5. 若目标设备含 **iOS 17.0–17.3.1**，两条路线都需要一个特权隧道进程（go-ios 内核 TUN 或 pmd3 tunneld），需在安装向导中单独提示 sudo；iOS 17.4+ 可完全免 root。

---

## 附：核心来源清单

- go-ios 仓库/README/Releases：https://github.com/danielpaulus/go-ios
- pymobiledevice3 仓库与文档站：https://github.com/doronz88/pymobiledevice3 ，https://doronz88.github.io/pymobiledevice3/
- pymobiledevice3 iOS 17+ 隧道指南（含支持矩阵）：https://doronz88.github.io/pymobiledevice3/guides/ios17-tunnels/
- libimobiledevice 官网与各仓库：https://libimobiledevice.org/ ，https://github.com/libimobiledevice/libimobiledevice ，https://github.com/libimobiledevice/usbmuxd ，https://github.com/libimobiledevice/ideviceinstaller
- iOS 17 影响的关键 issue：https://github.com/libimobiledevice/libimobiledevice/issues/1638 ，https://github.com/libimobiledevice/libimobiledevice/issues/1490 ，https://github.com/libimobiledevice/libimobiledevice/issues/1452 ，https://github.com/libimobiledevice/ideviceinstaller/issues/167
