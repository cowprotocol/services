/* Simple embeddable helper for linking to the local Playground Explorer.
 * Usage:
 *   <script src="http://localhost:8083/embed/debug-link.js"></script>
 *   <script>
 *     DebugInPlayground.config({ baseUrl: 'http://localhost:8083', fallbackUrl: 'https://etherscan.io' })
 *     document.body.appendChild(DebugInPlayground.link({ kind: 'tx', value: '0xabc...', label: 'Debug in Playground' }))
 *   </script>
 */
(function () {
  var BASE = 'http://localhost:8083';
  var FALLBACK = 'https://etherscan.io';
  
  function isHex(v, len) {
    return typeof v === 'string' && /^0x[0-9a-fA-F]+$/.test(v) && (!len || v.length === len);
  }
  
  function ensure0x(v) {
    return v && v.startsWith('0x') ? v : '0x' + String(v || '');
  }
  
  function setBase(u) {
    if (u) BASE = String(u).replace(/\/$/, '');
  }
  
  function setFallback(u) {
    if (u) FALLBACK = String(u).replace(/\/$/, '');
  }
  
  function txUrl(hash) {
    var h = String(hash || '');
    if (!isHex(h, 66)) return FALLBACK + '/tx/' + encodeURIComponent(h);
    return BASE + '/tx/' + h;
  }
  
  function addressUrl(addr) {
    var a = String(addr || '');
    if (!isHex(a, 42)) return FALLBACK + '/address/' + encodeURIComponent(a);
    return BASE + '/address/' + a.toLowerCase();
  }
  
  function blockUrl(id) {
    var n = typeof id === 'number' ? id : parseInt(String(id), 10);
    if (isNaN(n)) return BASE + '/block/latest';
    return BASE + '/block/' + n;
  }
  
  function link(opts) {
    opts = opts || {};
    var kind = opts.kind || 'tx';
    var value = opts.value || '';
    var label = opts.label || 'Debug in Playground';
    var className = opts.className || 'debug-in-playground-link';
    
    var a = document.createElement('a');
    a.className = className;
    a.textContent = label;
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
    
    if (kind === 'tx') {
      a.href = txUrl(value);
    } else if (kind === 'address') {
      a.href = addressUrl(value);
    } else if (kind === 'block') {
      a.href = blockUrl(value);
    } else {
      a.href = BASE;
    }
    
    return a;
  }
  
  function config(opts) {
    opts = opts || {};
    if (opts.baseUrl) setBase(opts.baseUrl);
    if (opts.fallbackUrl) setFallback(opts.fallbackUrl);
  }
  
  // Export the API
  window.DebugInPlayground = {
    config: config,
    txUrl: txUrl,
    addressUrl: addressUrl,
    blockUrl: blockUrl,
    link: link
  };
})();
