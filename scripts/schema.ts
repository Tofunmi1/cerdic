const child = Bun.spawn(['cargo', 'run', '--quiet', '-p', 've-graph', '--features', 'typescript',
  '--example', 'typescript', '--', process.argv.includes('--write') ? '--write' : '--check'],
  { cwd: new URL('../engine', import.meta.url).pathname, stdout: 'inherit', stderr: 'inherit' });
process.exit(await child.exited);
