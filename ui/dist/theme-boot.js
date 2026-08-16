/* Resolve persisted theme before first paint.
 * Uses localStorage (po.theme) with 'system' default.
 */
(function(){
  try {
    var p = localStorage.getItem('po.theme') || 'system';
    var r = p === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : p;
    document.documentElement.dataset.theme = r;
  } catch(e) {}
})();
