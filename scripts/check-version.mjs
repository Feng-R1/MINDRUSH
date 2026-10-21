import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const json = (path) => JSON.parse(readFileSync(new URL('../' + path, import.meta.url), 'utf8'));
const version = json('package.json').version;
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
assert.equal(cargo.match(/\[workspace\.package\]\s+version = "([^"]+)"/)?.[1], version);
assert.equal(json('frontend/package.json').version, version);
assert.equal(json('src-tauri/tauri.conf.json').version, version);
if (process.env.GITHUB_REF_TYPE === 'tag') {
  assert.equal(process.env.GITHUB_REF_NAME, 'v' + version);
}
console.log('All application versions match: ' + version);
