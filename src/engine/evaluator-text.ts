import type { Command, Diagnostic, Document } from './generated';

export type TextRead = { text?: string; error?: Diagnostic };
export type TextParse = { commands?: Command[]; spans?: unknown[]; diagnostics?: Diagnostic[]; error?: Diagnostic };
/** What one node of a program produced (see `Evaluator::trace`): operator, parameters, inputs, the kind and size of its value, and what it changed from its main input. */
export type NodeTrace = {
  id: string; label: string | null; operator: string; params: Record<string, unknown>;
  inputs: { port: string; from: string }[];
  kind: string; summary: string; facts: Record<string, unknown>; effect: string[]; warnings: string[];
};
export type TextReply = TextRead & TextParse & { lines?: Record<string, string>; nodes?: NodeTrace[] };

/** Stateless text requests share the evaluator worker without cancelling its geometry job. */
export class EvaluatorText {
  private next = 0;
  private pending = new Map<number, { resolve: (reply: TextReply) => void; reject: (error: Error) => void }>();

  constructor(private worker: () => Worker | null) {}

  request(document: Document, operation: 'compact' | 'nodes' | 'parse' | 'trace', text?: string): Promise<TextReply> {
    return new Promise((resolve, reject) => {
      const worker = this.worker();
      if (!worker) { reject(new Error('the evaluator is not running')); return; }
      const id = ++this.next;
      const timer = setTimeout(() => {
        this.pending.delete(id); reject(new Error('the text request took more than 20 seconds'));
      }, operation === 'trace' ? 120000 : 20000);
      this.pending.set(id, {
        resolve: reply => { clearTimeout(timer); resolve(reply); },
        reject: error => { clearTimeout(timer); reject(error); },
      });
      worker.postMessage({ kind: 'document_text', id, document, operation, text });
    });
  }

  receive(id: number, reply: TextReply) {
    this.pending.get(id)?.resolve(reply); this.pending.delete(id);
  }

  fail(message: string) {
    for (const request of this.pending.values()) request.reject(new Error(message));
    this.pending.clear();
  }
}
