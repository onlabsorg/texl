/*
 * Syntax highlighting for Swan (```swan) and Texl (```texl) code blocks.
 *
 * It wraps tokens in the same `hljs-*` classes that highlight.js uses, so the
 * colors come from texl.css. It runs on its own: the landing page loads it
 * without highlight.js, and in the manual it handles the two languages that
 * highlight.js does not know.
 */
(function () {
    "use strict";

    var BUILTINS = {
        bool: 1, numb: 1, text: 1, list: 1, nspace: 1, math: 1, time: 1,
        promise: 1, undef: 1, texl: 1, type: 1, parse: 1, bind: 1, label: 1
    };

    function escape(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }

    function span(cls, s) {
        return '<span class="hljs-' + cls + '">' + escape(s) + "</span>";
    }

    // Tokens of Swan source. Inside a template tag, comments and texts stop at
    // `%>`, which always ends the tag.
    var SWAN = /(#[^\n]*)|("[^"]*"?)|(\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)|([A-Za-z_][A-Za-z0-9_]*)|(->|=>|~>)/g;
    var SWAN_IN_TAG = /(#(?:(?!%>)[^\n])*)|("(?:(?!%>)[^"])*"?)|(\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)|([A-Za-z_][A-Za-z0-9_]*)|(->|=>|~>)/g;

    function swan(source, inTag) {
        var re = inTag ? SWAN_IN_TAG : SWAN;
        var out = "";
        var last = 0;
        var m;
        re.lastIndex = 0;
        while ((m = re.exec(source)) !== null) {
            out += escape(source.slice(last, m.index));
            if (m[1] !== undefined) {
                out += span("comment", m[1]);
            } else if (m[2] !== undefined) {
                out += span("string", m[2]);
            } else if (m[3] !== undefined) {
                out += span("number", m[3]);
            } else if (m[4] !== undefined) {
                var word = m[4];
                if (word === "TRUE" || word === "FALSE") {
                    out += span("literal", word);
                } else if (BUILTINS.hasOwnProperty(word) && source[m.index - 1] !== ".") {
                    out += span("built_in", word);
                } else {
                    out += escape(word);
                }
            } else {
                out += span("keyword", m[5]);
            }
            last = re.lastIndex;
        }
        return out + escape(source.slice(last));
    }

    function texl(source) {
        var out = "";
        var pos = 0;
        while (pos < source.length) {
            var start = source.indexOf("<%", pos);
            if (start < 0) {
                break;
            }
            var end = source.indexOf("%>", start + 2);
            if (end < 0) {
                break; // an unclosed tag is plain text
            }
            out += escape(source.slice(pos, start));
            out += span("template-tag", "<%");
            out += swan(source.slice(start + 2, end), true);
            out += span("template-tag", "%>");
            pos = end + 2;
        }
        return out + escape(source.slice(pos));
    }

    function highlightAll() {
        var blocks = document.querySelectorAll("code.language-swan, code.language-texl");
        Array.prototype.forEach.call(blocks, function (block) {
            var source = block.textContent;
            var isTexl = block.classList.contains("language-texl");
            block.innerHTML = isTexl ? texl(source) : swan(source, false);
            block.classList.add("hljs");
        });
    }

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", highlightAll);
    } else {
        highlightAll();
    }
})();
