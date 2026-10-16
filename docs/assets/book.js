/*
 * Adds the site navigation to mdBook's menu bar: the Texl wordmark, linking
 * to the landing page, and links to the main parts of the documentation.
 * `path_to_root` is defined by mdBook on every page.
 */
(function () {
    "use strict";

    /* global path_to_root */
    var book = typeof path_to_root === "string" ? path_to_root : "";
    var site = book + "../";

    var title = document.querySelector("#menu-bar .menu-title");
    if (title) {
        title.innerHTML =
            '<a class="tx-wordmark" href="' + site + 'index.html" title="Texl home">' +
            '<span class="tx-delim">&lt;%</span>texl<span class="tx-delim">%&gt;</span></a>';
    }

    var right = document.querySelector("#menu-bar .right-buttons");
    if (right) {
        var links = document.createElement("nav");
        links.className = "tx-book-links";
        links.setAttribute("aria-label", "Site");
        links.innerHTML =
            '<a class="tx-optional" href="' + book + 'templates/index.html">Templates</a>' +
            '<a class="tx-optional" href="' + book + 'swan/index.html">Swan</a>' +
            '<a href="' + book + 'hosts/index.html">API</a>';
        right.insertBefore(links, right.firstChild);
    }
})();
