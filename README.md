# nooboard

使用 Rust 开发的 macOS ↔ Windows 跨设备剪贴板工具，面向同一局域网或已有 VPN 中可直接连通的设备。

当前处于设计与工程准备阶段，尚未实现同步功能。

## 第一轮开发

先实现四个独立于界面的 Rust 库，通过自动化测试与最小命令行示例验证文字同步及本地文字历史：

```text
core → clipboard
     → network
     → storage
```

- `core`：组装底层模块，协调设备、同步与历史业务。
- `clipboard`：封装 macOS、Windows 原生剪贴板 API；本库的实现不依赖 arboard。
- `network`：连接、协议与经过身份验证的加密传输。
- `storage`：历史记录与配置持久化。

底层库之间不直接互相依赖。Tauri、React、TypeScript 桌面程序，以及图片和文件手动传输留到后续轮次。

## 设计文档

- [产品与架构设计](DESIGN.md)
- [第一轮开发目标与验收标准](MILESTONE-01.md)

建立 Cargo workspace 时加入 GitHub Actions，运行 Windows 与 macOS 的构建及自动化测试。真实桌面剪贴板和双设备 LAN/VPN 流程另做实机验收；Windows 远程访问配置不作为开工条件。
