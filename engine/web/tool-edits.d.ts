import type { Command, Document } from '../../src/engine/generated';
import type { TextReply } from '../../src/engine/evaluator-text';
type Doc = {
  state(): { document: Document };
  atRevision(revision: number): { document: Document; lines: Record<string, string>; error?: never } | { error: string; document?: never; lines?: never };
};
export function editDocument(params: Record<string, unknown>, services: {
  doc: Pick<Doc, 'state'>;
  parse(document: Document, text: string): Promise<TextReply>;
  mutate(commands: Command[], label: string, dryRun: boolean): Promise<unknown>;
}): Promise<unknown>;
export function diffDocument(params: Record<string, unknown>, services: {
  doc: Doc;
}): Promise<unknown>;
