// The Vercel function: a thin Node adapter that turns the incoming request into a standard Request for `core` and writes its Response back.
import type { IncomingMessage, ServerResponse } from 'node:http';
import { createRelay, restStore } from './core';

const url = process.env.KV_REST_API_URL ?? process.env.UPSTASH_REDIS_REST_URL ?? '';
const token = process.env.KV_REST_API_TOKEN ?? process.env.UPSTASH_REDIS_REST_TOKEN ?? '';
const handle = createRelay(restStore(url, token));

export default async function handler(req: IncomingMessage, res: ServerResponse) {
  if (!url || !token) { res.statusCode = 503; res.setHeader('content-type', 'application/json'); res.end('{"error":"The relay has no store configured."}'); return; }
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  const headers = new Headers();
  for (const [name, value] of Object.entries(req.headers)) if (value !== undefined) headers.set(name, Array.isArray(value) ? value.join(', ') : value);
  const method = req.method ?? 'GET';
  const request = new Request(`https://${req.headers.host ?? 'relay.local'}${req.url ?? '/'}`, { method, headers, ...(method === 'GET' || method === 'HEAD' ? {} : { body: Buffer.concat(chunks) }) });
  const response = await handle(request);
  res.statusCode = response.status;
  response.headers.forEach((value, name) => res.setHeader(name, value));
  res.end(Buffer.from(await response.arrayBuffer()));
}
