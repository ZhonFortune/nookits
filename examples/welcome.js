function (main) {
  // 在此处编写您的代码。支持 async function (main) 和 main.require 加载 Node.js 模块。
  const fs = main.require('node:fs');
  const path = main.require('node:path');
  fs.writeFileSync(path.join(main.dataDir, 'welcome.txt'), 'Nookits is ready.\n', 'utf8');
}
