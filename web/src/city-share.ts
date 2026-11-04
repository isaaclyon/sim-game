// Share links (ADR 0035): the save bytes, deflated and base64url-encoded, in the URL fragment.
// Fragments never reach a server, so a link carries the whole city and nothing is uploaded.

const KEY = 'city=';
/** Longer links are refused; browsers and chat apps cut or reject very long URLs. */
export const MAX_LINK_LENGTH = 1_000_000;

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream) {
  const body = new Blob([bytes as Uint8Array<ArrayBuffer>]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(body).arrayBuffer());
}

function toBase64Url(bytes: Uint8Array) {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000)
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

function fromBase64Url(text: string) {
  const binary = atob(text.replaceAll('-', '+').replaceAll('_', '/'));
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

/** A link to `page` that opens this city, or null when it would be too long to share. */
export async function cityLink(bytes: Uint8Array, page: string): Promise<string | null> {
  const packed = toBase64Url(await pipe(bytes, new CompressionStream('deflate-raw')));
  const link = `${page.split('#')[0]}#${KEY}${packed}`;
  return link.length > MAX_LINK_LENGTH ? null : link;
}

/** The save bytes in a URL fragment, or null if it holds no city. Throws if damaged. The sim
 * still validates the bytes before anything is replaced. */
export async function cityFromLink(fragment: string): Promise<Uint8Array | null> {
  const body = fragment.replace(/^#/, '');
  if (!body.startsWith(KEY)) return null;
  try {
    const packed = fromBase64Url(body.slice(KEY.length));
    return await pipe(packed, new DecompressionStream('deflate-raw'));
  } catch {
    throw new Error(
      'This city link is damaged or incomplete. Ask for a new link or the city file.',
    );
  }
}
