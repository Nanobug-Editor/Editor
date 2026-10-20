/** Unknown-language stdio fixture: selected resolve and explicit related-source membership only. */
const fs = require('node:fs');
const [log, delay] = process.argv.slice(2);
const documents = new Map();
let input = Buffer.alloc(0);

/** Record complete replies before emitting their standard byte-framed wire representation. */
function reply(id, result) {
  fs.appendFileSync(log, JSON.stringify({ direction: 'response', id, result }) + '\n');
  const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', id, result }));
  process.stdout.write(`Content-Length: ${body.length}\r\n\r\n`);
  process.stdout.write(body);
}

/** JavaScript offsets and LSP columns both count UTF-16 code units, including surrogate pairs. */
function position(source, offset) {
  const lines = source.slice(0, offset).split('\n');
  return { line: lines.length - 1, character: lines.at(-1).length };
}

/** This backend explicitly selects all open fixture sources; the host has no filename graph rule. */
function handle(message) {
  fs.appendFileSync(log, JSON.stringify(message) + '\n');
  const { method, params, id } = message;
  if (method === 'textDocument/didOpen') documents.set(params.textDocument.uri, params.textDocument.text);
  if (method === 'textDocument/didChange') documents.set(params.textDocument.uri, params.contentChanges.at(-1).text);
  if (method === 'textDocument/didClose') documents.delete(params.textDocument.uri);
  if (id === undefined) return;
  if (method === 'initialize') return reply(id, { capabilities: {
    textDocumentSync: 1, completionProvider: { resolveProvider: true },
    experimental: { nanobugControl: { version: 1, methods: ['fixture/context/documents'] } }
  } });
  if (method === 'shutdown') return reply(id, null);
  if (method === 'fixture/context/documents') return reply(id, {
    generation: 'completion-fixture-graph', documents: params.arguments.candidates.map(value => value.id)
  });
  const source = documents.get(params?.textDocument?.uri) || '';
  let result = null;
  if (method === 'textDocument/completion') result = [{ label: 'resolvedSymbol', data: {
    uri: params.textDocument.uri, invalid: source.startsWith('// invalid')
  } }];
  if (method === 'completionItem/resolve') {
    const original = documents.get(params.data.uri) || '';
    const range = { start: position(original, original.length - 3), end: position(original, original.length) };
    result = { ...params, textEdit: { range, newText: 'resolvedSymbol' }, additionalTextEdits: [
      { range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } }, newText: "import { resolvedSymbol } from './module';\r\n" }
    ] };
    // The valid import and main edit must remain unapplied if a later edit overlaps the main token.
    if (params.data.invalid) result.additionalTextEdits.push({ range, newText: 'illegal partial tail' });
  }
  setTimeout(() => reply(id, result), Number(delay));
}

/** Several complete frames can share a read; an incomplete tail remains untouched for the next one. */
process.stdin.on('data', chunk => {
  input = Buffer.concat([input, chunk]);
  for (;;) {
    const boundary = input.indexOf('\r\n\r\n');
    if (boundary < 0) return;
    const match = /Content-Length:\s*(\d+)/i.exec(input.subarray(0, boundary).toString());
    if (!match) process.exit(2);
    const length = Number(match[1]);
    if (input.length < boundary + 4 + length) return;
    const body = input.subarray(boundary + 4, boundary + 4 + length);
    input = input.subarray(boundary + 4 + length);
    handle(JSON.parse(body));
  }
});
