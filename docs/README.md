# nooboard 文档

## 使用 nooboard

- [下载安装与快速开始](../README.md)
- [使用指南](user-guide.md)：配对、文字同步、图片和文件、历史、后台运行。
- [常见问题](troubleshooting.md)：网络、系统兼容性、保存目录和错误处理。

## 开发 nooboard

- [开发与贡献](development.md)：环境、构建、目录职责和发布流程。
- [测试与实机验收](testing.md)：自动测试、隔离的原生测试及跨机验收。
- [桌面架构](../apps/desktop/ARCHITECTURE.md)：前端边界、Tauri 宿主、IPC 和预览。
- [Terminal Service 重构设计](terminal-service-design.md)：新的配置、依赖调度、运行数据、服务接口和停止规则，尚未实现。
- 库接口参考：[core](../crates/core/README.md)、[clipboard](../crates/clipboard/README.md)、[network](../crates/network/README.md)、[storage](../crates/storage/README.md)。
- [品牌与动画资源](../apps/desktop/public/mascot/README.md)：素材来源、导出和动画契约。

根 README 与使用指南面向应用用户。构建命令、内部类型、架构约束和验收记录放在开发文档；crate README 面向库调用者，部分内容也直接用于生成 Rust API 文档。
