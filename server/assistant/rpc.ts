import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createInterface } from 'node:readline';

type Pending = { resolve: (value: any) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> };
/** Local app-server transport. Account credentials stay inside Codex. */
export class CodexRpc {
  private process: ChildProcessWithoutNullStreams;
  private nextId = 0;
  private pending = new Map<number, Pending>();
  onMessage: (message: any) => void = () => {};
  onExit: () => void = () => {};
  constructor() {
    this.process = spawn(process.env.CERDIC_CODEX_BIN ?? 'codex', ['app-server', '-c', 'mcp_servers.cerdic.enabled=false', '-c', 'mcp_servers.blender.enabled=false', '-c', 'features.apps=false'], { stdio: ['pipe', 'pipe', 'pipe'] });
    createInterface({ input: this.process.stdout }).on('line', line => {
      try {
        const message = JSON.parse(line);
        if (message.id !== undefined && !message.method) {
          const task = this.pending.get(message.id);
          if (!task) return;
          clearTimeout(task.timer); this.pending.delete(message.id);
          if (message.error) task.reject(new Error(message.error.message ?? 'Codex request failed'));
          else task.resolve(message.result);
        } else this.onMessage(message);
      } catch { /* Non-protocol stdout cannot become a browser event. */ }
    });
    this.process.stderr.on('data', () => {});
    this.process.on('error', error => this.fail(new Error(`Could not start Codex: ${error.message}`)));
    this.process.on('exit', () => { this.fail(new Error('Local Codex disconnected')); this.onExit(); });
  }
  private fail(error: Error) { for (const task of this.pending.values()) { clearTimeout(task.timer); task.reject(error); } this.pending.clear(); }
  send(value: unknown) { if (!this.process.stdin.destroyed) this.process.stdin.write(JSON.stringify(value) + '\n'); }
  request(method: string, params: unknown = {}, timeout = 60000): Promise<any> {
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`${method} timed out`)); }, timeout);
      this.pending.set(id, { resolve, reject, timer }); this.send({ id, method, params });
    });
  }
  close() { this.fail(new Error('Connection closed')); this.process.kill(); }
}
