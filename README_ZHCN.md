# Nookits

[English](README.md) | 简体中文

<img src="assets/icon.svg" alt="Nookits" width="96" />

**常用脚本，一触即达。**

Nookits 是一款常驻 Windows 系统托盘的轻量脚本工具。把常用 JavaScript 脚本放进文件夹，就能从托盘菜单一键运行，方便处理生成密码、整理文件等日常任务。

- 无需安装任何运行环境，直接运行脚本。
- 本地使用，数据保存在自己的电脑上。
- 从托盘菜单直接运行脚本。
- 添加、重命名或删除脚本后，菜单自动更新。
- 支持多语言和开机启动。

## 示例默认脚本

| 样例                      | 功能                                               |
| ------------------------- | -------------------------------------------------- |
| Gen 64Bit Token           | 生成 64 个字符的大小写字母与数字密钥，复制到剪贴板 |
| Gen 16Bit Random Password | 生成 16 个字符的大小写字母密码，复制到剪贴板       |

## 添加自己的脚本

打开脚本目录，新建一个 `.js` 文件，以自己希望在菜单中看到的名称命名，例如 `记录时间.js`：

```javascript
function (main) {
  const fs = main.require('node:fs');
  const path = main.require('node:path');
  fs.writeFileSync(
    path.join(main.dataDir, 'time.txt'),
    new Date().toLocaleString(),
    'utf8',
  );
}
```

保存后，托盘菜单会出现 **记录时间**。点击运行，即可在 `.nookits/` 中生成 `time.txt`。

脚本使用 `function (main) { }` 作为入口，也支持 `async function (main) { }`。`main.dataDir` 是应用数据目录，`main.scriptsDir` 是脚本目录，`main.require()` 用于加载模块。更多示例见 [assets/example](assets/example)。

脚本修改后，下次运行即生效。删除脚本即可从菜单中移除，后续启动会保留这些修改。请仅运行可信脚本；运行失败时，可查看 `.nookits/runtime.log`。

## 本地开发

```powershell
npm install
npm run dev
```

## 许可证

本项目采用 [MIT 许可证](LICENSE.md)。
