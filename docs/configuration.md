# Nookit 配置

数据目录保持为用户主目录下的 `.nookits/`，首次启动只创建空的 `scripts/` 目录与设置文件，不写入示例脚本。

## 关于与仓库地址

固定菜单“关于”显示软件名称、版本和简介。通过构建环境变量 `NOOKIT_GITHUB_URL` 注入 GitHub 仓库地址，配置后显示“访问 GitHub”按钮。地址要求为 `https://github.com/所有者/仓库`。

在 PowerShell 中设置 `$env:NOOKIT_GITHUB_URL` 后执行 `npm run dev` 或 `npm run build`。地址编译进应用，分发后无需用户配置环境变量。未设置时仅显示软件信息。修改变量后需重新启动开发命令。

## 运行器与图标

脚本运行器源码为 `scripts/runtime.mjs`。开发模式直接加载该文件；发布时复制到应用资源目录，并携带 jiti 和 Node.js。

图标源文件为 `assets/icon.svg`。开发和打包准备阶段使用现有 Tauri CLI 生成 Windows ICO 与各尺寸图标。托盘、Windows 可执行文件资源和安装包使用同源图标。Windows 产品名称与文件描述为 Nookit，主进程文件名为 `Nookit.exe`。Node.js 子进程仅在运行脚本时出现。
