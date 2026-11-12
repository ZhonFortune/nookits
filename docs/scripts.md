# 脚本与开发

Nookits 是纯系统托盘应用，无主窗口。首次启动创建用户主目录下的 `.nookits/scripts/`，设置与日志保存在 `.nookits/`。示例仅在脚本目录首次创建时生成。

脚本目录仅扫描顶层 `.js` 文件，菜单显示去除扩展名的文件名，按名称排序。新增、重命名和删除会自动刷新。

```javascript
async function (main) {
  const fs = main.require('node:fs/promises');
  const path = main.require('node:path');
  await fs.writeFile(path.join(main.dataDir, 'result.txt'), '完成', 'utf8');
}
```

文件内容为一个函数表达式。`main` 提供 `dataDir`、`scriptsDir`、`scriptPath` 与相对于脚本解析的 `require`。可用 Node.js 内置模块；外部依赖需存在于脚本目录可解析的 `node_modules`。异步入口会等待返回的 Promise。

运行器采用 npm 库 jiti，每次点击启动独立 Node.js 进程，同一脚本运行期间禁止重复启动。单次运行最长 300 秒，超时或退出应用时终止运行器；脚本应自行管理它创建的子进程。脚本具有当前用户的文件与系统访问权限，仅运行可信脚本。输出与错误写入 `runtime.log`，下次启动脚本时超过 1 MiB 的日志会清空。

翻译位于 `locales/en.json` 与 `locales/zh-CN.json`。语言设置保存到 `settings.json`；开机启动使用系统实际状态。默认简体中文。

开发环境需要 Node.js 22 或更新版本、Rust 与 Tauri 2 的系统构建前置依赖。先执行 `npm install`，之后 `npm run dev`。准备脚本将当前 Node.js 可执行文件和 jiti 拷贝到打包目录，Tauri 开发模式会监听 Rust、翻译及运行器源码。开发运行器直接加载源文件，修改后下次执行脚本即可使用最新逻辑。脚本目录中的脚本保存后下次运行直接使用最新内容。

`npm run build` 生成 Windows NSIS 安装包，携带 Node.js 运行时，用户无需安装 Node.js。构建架构使用本机 Rust host 与本机 Node.js 架构，二者必须一致。当前配置面向 Windows；运行时体积主要由携带的 Node.js 决定，空闲时只运行 Rust 托盘进程。
