# Nookits

<img src="assets/icon.svg" alt="Nookits" width="96" />

<br>English | [简体中文](README_ZHCN.md)<br>

**Your scripts, one click away.**

Nookits is a lightweight script tool that lives in the Windows system tray. Drop your JavaScript files into a folder and run them from the tray menu to handle everyday tasks such as generating passwords or organizing files.

- Run scripts without installing a separate runtime.
- Keep your data locally on your own computer.
- Launch scripts directly from the tray menu.
- See menu updates automatically when scripts are added, renamed, or removed.
- Choose your language and enable launch at login.

## Included Scripts

| Script                    | What it does                                                                                                                |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Gen 64Bit Token           | Generates a 64-character token containing uppercase letters, lowercase letters, and digits, then copies it to the clipboard |
| Gen 16Bit Random Password | Generates a 16-character password containing uppercase and lowercase letters, then copies it to the clipboard               |

## Add Your Own Scripts

Open the scripts folder from the tray menu and create a `.js` file. Its filename becomes its menu label. For example, create `Record Time.js`:

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

After saving, **Record Time** appears in the tray menu. Click it to create `time.txt` in `.nookits/` under your home directory.

Use `function (main) { }` as the entry point, or `async function (main) { }` for asynchronous scripts. `main.dataDir` is the application data folder, `main.scriptsDir` is the scripts folder, and `main.require()` loads modules. For more examples, see [assets/example](assets/example).

Edits take effect the next time you run a script. Delete a script to remove it from the menu; your changes are preserved across restarts. Only run scripts you trust. If a script fails, check `.nookits/runtime.log`.

## Local Development

```powershell
npm install
npm run dev
```

## License

This project is licensed under the [MIT License](LICENSE.md).
