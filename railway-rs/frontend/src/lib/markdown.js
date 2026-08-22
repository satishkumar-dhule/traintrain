/* markdown.js - tiny, dependency-free Markdown -> HTML renderer for AI
   answers. Security model: every raw text fragment is HTML-escaped BEFORE any
   inline markup is applied, URLs must match an allow-list regex before they
   become hrefs, and code blocks/inline code are escaped verbatim. Output is
   therefore safe to inject with Svelte's {@html}.

   Supported subset (what the models actually emit):
     # / ## / ### headings          - bullet lists (-, *)
     fenced code blocks  ```lang    1. numbered lists
     tables |a|b| with |---| sep    > blockquotes
     **bold**, *italic*, `code`     [text](https://…) links
     --- horizontal rules           paragraphs with soft line breaks

   Dual-exported like static/routes.js so Node tests can require() it. */

function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  var SAFE_URL = /^(https?:\/\/|mailto:|tel:)[^\s"'<>]+$/i;

  function safeHref(url) {
    return SAFE_URL.test(url) ? url : null;
  }

  /* Inline pass on already-escaped text. Order matters: inline code first so
     emphasis markers inside backticks stay literal. */
function inline(s) {
    var codes = [];
    s = s.replace(/`([^`\n]+)`/g, function (_, c) {
      codes.push(c);
      return '\u0000' + (codes.length - 1) + '\u0000';
    });
    s = s.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, function (_, text, href) {
      var safe = safeHref(href);
      if (!safe) return text;
      return '<a href="' + escapeHtml(safe) + '" target="_blank" rel="noopener noreferrer">' + text + '</a>';
    });
    s = s.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
    s = s.replace(/(^|[\s(])\*([^*\n]+)\*(?=$|[\s.,;:!?)])/g, '$1<em>$2</em>');
    s = s.replace(/(^|[\s(])_([^_\n]+)_(?=$|[\s.,;:!?)])/g, '$1<em>$2</em>');
    s = s.replace(/\u0000(\d+)\u0000/g, function (_, i) {
      return '<code class="md-code">' + codes[Number(i)] + '</code>';
    });
    return s;
  }

function isTableSep(line) {
    if (!line || line[0] !== '|') return false;
    return /^\|[\s:|-]+\|$/.test(line.trim());
  }

function splitRow(line) {
    return line
      .trim()
      .replace(/^\|/, '')
      .replace(/\|$/, '')
      .split('|')
      .map(function (c) { return c.trim(); });
  }

  /* Block parser over raw (unescaped) lines; escaping happens per text node. */
function renderMarkdown(src) {
    var lines = String(src == null ? '' : src).replace(/\r\n?/g, '\n').split('\n');
    var out = [];
    var i = 0;

  function flushParagraph(buf) {
      if (buf.length) {
        /* Escape first, then apply inline markup — never the other way. */
        out.push('<p class="md-p">' +
          buf.map(escapeHtmlFirst).join('<br/>') +
          '</p>');
      }
    }

    /* Escape then inline for one paragraph line. */
  function escapeHtmlFirst(line) {
      return inline(escapeHtml(line));
    }

    var para = [];
    while (i < lines.length) {
      var line = lines[i];

      // Fenced code block.
      var fence = /^```(\w*)\s*$/.exec(line.trim());
      if (fence) {
        flushParagraph(para); para = [];
        var body = [];
        i++;
        while (i < lines.length && !/^```\s*$/.test(lines[i].trim())) {
          body.push(lines[i]);
          i++;
        }
        i++; // closing fence (or EOF)
        out.push('<pre class="md-pre"><code>' + escapeHtml(body.join('\n')) + '</code></pre>');
        continue;
      }

      // Heading.
      var h = /^(#{1,4})\s+(.*)$/.exec(line);
      if (h) {
        flushParagraph(para); para = [];
        var level = Math.min(h[1].length + 2, 6); // ## -> h4 in chat bubbles
        out.push('<h' + level + ' class="md-h">' + inline(escapeHtml(h[2].trim())) + '</h' + level + '>');
        i++;
        continue;
      }

      // Horizontal rule.
      if (/^\s*([-*_])\s*(\1\s*){2,}$/.test(line)) {
        flushParagraph(para); para = [];
        out.push('<hr class="md-hr"/>');
        i++;
        continue;
      }

      // Table: header row + separator row.
      if (line[0] === '|' && i + 1 < lines.length && isTableSep(lines[i + 1])) {
        flushParagraph(para); para = [];
        var header = splitRow(line);
        i += 2;
        var rows = [];
        while (i < lines.length && lines[i][0] === '|') {
          rows.push(splitRow(lines[i]));
          i++;
        }
        var html = '<div class="md-table-wrap"><table class="md-table"><thead><tr>';
        header.forEach(function (c) {
          html += '<th>' + inline(escapeHtml(c)) + '</th>';
        });
        html += '</tr></thead><tbody>';
        rows.forEach(function (r) {
          html += '<tr>';
          header.forEach(function (_, ci) {
            html += '<td>' + inline(escapeHtml(r[ci] || '')) + '</td>';
          });
          html += '</tr>';
        });
        html += '</tbody></table></div>';
        out.push(html);
        continue;
      }

      // Bullet list.
      var ul = /^\s*[-*+]\s+(.*)$/.exec(line);
      if (ul) {
        flushParagraph(para); para = [];
        var items = [];
        while (i < lines.length) {
          var m = /^\s*[-*+]\s+(.*)$/.exec(lines[i]);
          if (!m) break;
          items.push(m[1]);
          i++;
        }
        out.push('<ul class="md-ul">' + items.map(function (t) {
          return '<li>' + inline(escapeHtml(t)) + '</li>';
        }).join('') + '</ul>');
        continue;
      }

      // Numbered list.
      var ol = /^\s*\d+[.)]\s+(.*)$/.exec(line);
      if (ol) {
        flushParagraph(para); para = [];
        var nums = [];
        while (i < lines.length) {
          var n = /^\s*\d+[.)]\s+(.*)$/.exec(lines[i]);
          if (!n) break;
          nums.push(n[1]);
          i++;
        }
        out.push('<ol class="md-ol">' + nums.map(function (t) {
          return '<li>' + inline(escapeHtml(t)) + '</li>';
        }).join('') + '</ol>');
        continue;
      }

      // Blockquote.
      var q = /^\s*>\s?(.*)$/.exec(line);
      if (q) {
        flushParagraph(para); para = [];
        var quote = [];
        while (i < lines.length) {
          var qq = /^\s*>\s?(.*)$/.exec(lines[i]);
          if (!qq) break;
          quote.push(qq[1]);
          i++;
        }
        out.push('<blockquote class="md-quote">' + quote.map(escapeHtmlFirst).join('<br/>') + '</blockquote>');
        continue;
      }

      // Blank line ends the current paragraph.
      if (line.trim() === '') {
        flushParagraph(para); para = [];
        i++;
        continue;
      }

      para.push(line);
      i++;
    }
    flushParagraph(para);
    return out.join('');
  }

export { renderMarkdown, escapeHtml };
