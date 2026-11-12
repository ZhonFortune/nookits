function (main) {
  const { randomInt } = main.require('node:crypto');
  const { execFileSync } = main.require('node:child_process');
  const length = 16;
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

  if (process.platform !== 'win32') {
    throw new Error('This example requires the Windows clipboard.');
  }

  let password;
  do {
    password = Array.from({ length }, () => alphabet[randomInt(alphabet.length)]).join('');
  } while (!/[A-Z]/.test(password) || !/[a-z]/.test(password));

  execFileSync('powershell.exe', [
    '-NoLogo',
    '-NoProfile',
    '-NonInteractive',
    '-STA',
    '-Command',
    'Set-Clipboard -Value ([Console]::In.ReadToEnd()) -ErrorAction Stop',
  ], {
    input: password,
    encoding: 'utf8',
    windowsHide: true,
    timeout: 10000,
    stdio: ['pipe', 'ignore', 'pipe'],
  });
}
