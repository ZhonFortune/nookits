function (main) {
  const { randomInt } = main.require('node:crypto');
  const { execFileSync } = main.require('node:child_process');
  const length = 64;
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';

  if (process.platform !== 'win32') {
    throw new Error('This example requires the Windows clipboard.');
  }

  let token;
  do {
    token = Array.from({ length }, () => alphabet[randomInt(alphabet.length)]).join('');
  } while (!/[A-Z]/.test(token) || !/[a-z]/.test(token) || !/[0-9]/.test(token));

  execFileSync('powershell.exe', [
    '-NoLogo',
    '-NoProfile',
    '-NonInteractive',
    '-STA',
    '-Command',
    'Set-Clipboard -Value ([Console]::In.ReadToEnd()) -ErrorAction Stop',
  ], {
    input: token,
    encoding: 'utf8',
    windowsHide: true,
    timeout: 10000,
    stdio: ['pipe', 'ignore', 'pipe'],
  });
}
