//! The song history page: one self-contained HTML file written next to the data and opened in the
//! default browser. Everything is inline - CSS, JS, the data as JSON - so it works from `file://`
//! with no network beyond the cover-art thumbnails.
//!
//! Hiding a row is client-side (`localStorage`) in this version; the tray submenu does not see it.
//! A native window later would own that state properly.
//!
//! Not a company tool, so not the yellow-room theme: near-black, neon magenta and cyan, a vaporwave
//! sunset behind the title and a perspective grid under it, scanlines over everything.

use crate::songs::{self, Grouped};
use std::path::PathBuf;

pub fn path() -> PathBuf {
    crate::config::Config::dir().join("songs.html")
}

pub fn open() -> anyhow::Result<()> {
    let rows = songs::grouped(&songs::load());
    std::fs::create_dir_all(crate::config::Config::dir())?;
    std::fs::write(path(), render(&rows))?;
    crate::win::overlay::open_path(&path())
}

#[derive(serde::Serialize)]
struct Row<'a> {
    key: &'a str,
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    cover: &'a str,
    first: i64,
    last: i64,
    count: u32,
    spotify: String,
    apple: String,
    youtube: String,
    shazam: String,
}

pub fn render(rows: &[Grouped]) -> String {
    let data: Vec<Row> = rows
        .iter()
        .map(|g| Row {
            key: &g.find.shazam_key,
            title: &g.find.title,
            artist: &g.find.artist,
            album: g.find.album.as_deref().unwrap_or(""),
            cover: g.find.cover_url.as_deref().unwrap_or(""),
            first: g.first,
            last: g.last,
            count: g.count,
            spotify: songs::spotify_url(&g.find),
            apple: songs::apple_url(&g.find),
            youtube: songs::youtube_url(&g.find),
            shazam: songs::shazam_url(&g.find),
        })
        .collect();
    // `</` -> `<\/` is the one escape that makes JSON safe inside a <script> block.
    let json = serde_json::to_string(&data)
        .unwrap_or_else(|_| "[]".into())
        .replace("</", "<\\/");
    TEMPLATE
        .replace("/*DATA*/", &json)
        .replace("/*COUNT*/", &rows.len().to_string())
        .replace("/*VERSION*/", env!("CARGO_PKG_VERSION"))
}

const TEMPLATE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>SONG HISTORY</title>
<style>
:root{--bg:#0b0710;--panel:#140b1f;--ink:#e8e6f0;--dim:#8b86a3;--mag:#ff2bd6;--cyan:#19e6ff;--amber:#ffb347;--line:#2a1a3d}
*{box-sizing:border-box}
html,body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 "Cascadia Code","Consolas",ui-monospace,monospace}
body::before{content:"";position:fixed;inset:0;pointer-events:none;z-index:9;
 background:repeating-linear-gradient(to bottom,rgba(255,255,255,.025) 0 1px,transparent 1px 3px)}
header{position:relative;isolation:isolate;padding:34px 28px 26px;overflow:hidden;border-bottom:1px solid var(--line)}
header::before{content:"";position:absolute;inset:-40% -10% auto -10%;height:240%;z-index:-1;
 background:radial-gradient(ellipse at 50% 110%,#ff5e7e 0%,#b0359a 22%,#4b1e7a 46%,transparent 70%);opacity:.75;filter:blur(18px)}
header::after{content:"";position:absolute;left:0;right:0;bottom:0;height:120px;z-index:-1;opacity:.5;
 background:linear-gradient(transparent 0,var(--bg) 100%),
 repeating-linear-gradient(90deg,var(--cyan) 0 1px,transparent 1px 48px),
 repeating-linear-gradient(0deg,var(--cyan) 0 1px,transparent 1px 24px);
 transform:perspective(300px) rotateX(60deg);transform-origin:bottom}
h1{margin:0;font-size:32px;letter-spacing:.35em;font-weight:700;color:#fff;
 text-shadow:0 0 6px var(--mag),0 0 22px var(--mag),2px 0 0 var(--cyan),-2px 0 0 var(--mag)}
.sub{margin-top:8px;color:var(--cyan);letter-spacing:.2em;font-size:12px;text-transform:uppercase}
.bar{display:flex;gap:14px;align-items:center;padding:16px 28px;border-bottom:1px solid var(--line);background:var(--panel)}
input{flex:1;max-width:520px;background:#0e0817;border:1px solid var(--line);color:var(--ink);padding:10px 14px;font:inherit;outline:none;border-radius:3px}
input:focus{border-color:var(--cyan);box-shadow:0 0 0 1px var(--cyan),0 0 18px rgba(25,230,255,.35)}
.count{color:var(--dim);letter-spacing:.1em}
table{width:100%;border-collapse:collapse}
th{position:sticky;top:0;background:var(--panel);color:var(--cyan);text-align:left;font-weight:600;letter-spacing:.12em;text-transform:uppercase;font-size:11px;padding:12px 14px;border-bottom:1px solid var(--line);cursor:pointer;user-select:none}
th.on{color:var(--mag)}
th.on::after{content:" \25BE";}th.on.asc::after{content:" \25B4"}
td{padding:10px 14px;border-bottom:1px solid var(--line);vertical-align:middle}
tr:hover td{background:rgba(255,43,214,.06)}
.cover{width:48px;height:48px;object-fit:cover;border:1px solid var(--cyan);box-shadow:0 0 10px rgba(25,230,255,.35);background:#000;display:block}
.cover.none{display:grid;place-items:center;color:var(--dim);font-size:10px}
.title{color:#fff;font-weight:600}
.dim{color:var(--dim)}
.links{display:flex;gap:6px;flex-wrap:wrap}
.links a{display:inline-block;padding:5px 9px;border:1px solid;border-radius:3px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;text-decoration:none;transition:box-shadow .15s,transform .15s}
.links a:hover{transform:translateY(-1px)}
a.sp{color:#1ed760;border-color:#1ed760}a.sp:hover{box-shadow:0 0 14px rgba(30,215,96,.6)}
a.ap{color:#ff6b8a;border-color:#ff6b8a}a.ap:hover{box-shadow:0 0 14px rgba(255,107,138,.6)}
a.yt{color:#ff5555;border-color:#ff5555}a.yt:hover{box-shadow:0 0 14px rgba(255,85,85,.6)}
a.sz{color:var(--cyan);border-color:var(--cyan)}a.sz:hover{box-shadow:0 0 14px rgba(25,230,255,.6)}
button.hide{background:none;border:1px solid var(--line);color:var(--dim);padding:4px 8px;cursor:pointer;font:inherit;font-size:11px}
button.hide:hover{color:var(--amber);border-color:var(--amber)}
.empty{padding:60px;text-align:center;color:var(--dim);letter-spacing:.2em;text-transform:uppercase}
footer{padding:18px 28px;color:var(--dim);font-size:11px;letter-spacing:.1em;display:flex;justify-content:space-between}
footer a{color:var(--cyan)}
</style></head>
<body>
<header><h1>SONG HISTORY</h1><div class="sub">taskbar-eq // shazam identifications</div></header>
<div class="bar"><input id="q" type="search" placeholder="search title / artist / album" autofocus><span class="count" id="count"></span></div>
<table id="t"><thead><tr>
<th data-k="" style="width:64px"></th>
<th data-k="title">Title</th><th data-k="artist">Artist</th><th data-k="album">Album</th>
<th data-k="first">First heard</th><th data-k="last">Last heard</th><th data-k="count" style="width:70px">Times</th>
<th data-k="" style="width:330px">Listen</th><th data-k="" style="width:60px"></th>
</tr></thead><tbody id="rows"></tbody></table>
<div class="empty" id="empty" hidden>no songs yet</div>
<footer><span>/*COUNT*/ songs &middot; v/*VERSION*/</span><span><a href="#" id="unhide">show hidden</a></span></footer>
<script id="data" type="application/json">/*DATA*/</script>
<script>
const DATA=JSON.parse(document.getElementById('data').textContent);
const HKEY='taskbar-eq.hidden';
let hidden=new Set(JSON.parse(localStorage.getItem(HKEY)||'[]'));
let sortK='last',asc=false,q='';
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const when=t=>new Date(t*1000).toLocaleString(undefined,{dateStyle:'medium',timeStyle:'short'});
function render(){
  const ql=q.toLowerCase();
  let rows=DATA.filter(r=>!hidden.has(r.key)&&(r.title+' '+r.artist+' '+r.album).toLowerCase().includes(ql));
  rows.sort((a,b)=>{let x=a[sortK],y=b[sortK];if(typeof x==='string'){x=x.toLowerCase();y=y.toLowerCase()}return (x<y?-1:x>y?1:0)*(asc?1:-1)});
  document.getElementById('rows').innerHTML=rows.map(r=>`<tr>
<td>${r.cover?`<img class="cover" loading="lazy" src="${esc(r.cover)}" alt="" onerror="this.outerHTML='<div class=&quot;cover none&quot;>no art</div>'">`:`<div class="cover none">no art</div>`}</td>
<td class="title">${esc(r.title)}</td><td>${esc(r.artist)}</td><td class="dim">${esc(r.album)}</td>
<td class="dim">${when(r.first)}</td><td>${when(r.last)}</td><td>${r.count}</td>
<td><div class="links"><a class="sp" href="${esc(r.spotify)}" target="_blank" rel="noopener">Spotify</a><a class="ap" href="${esc(r.apple)}" target="_blank" rel="noopener">Apple</a><a class="yt" href="${esc(r.youtube)}" target="_blank" rel="noopener">YouTube</a><a class="sz" href="${esc(r.shazam)}" target="_blank" rel="noopener">Shazam</a></div></td>
<td><button class="hide" data-k="${esc(r.key)}" title="hide this song on this browser">hide</button></td></tr>`).join('');
  document.getElementById('count').textContent=rows.length+' / '+DATA.length+(hidden.size?' ('+hidden.size+' hidden)':'');
  document.getElementById('empty').hidden=rows.length>0;
  document.querySelectorAll('th').forEach(th=>{th.classList.toggle('on',th.dataset.k===sortK);th.classList.toggle('asc',asc)});
}
document.getElementById('q').addEventListener('input',e=>{q=e.target.value;render()});
document.querySelectorAll('th[data-k]').forEach(th=>th.addEventListener('click',()=>{const k=th.dataset.k;if(!k)return;if(sortK===k)asc=!asc;else{sortK=k;asc=(k==='title'||k==='artist'||k==='album')}render()}));
document.getElementById('rows').addEventListener('click',e=>{const b=e.target.closest('button.hide');if(!b)return;hidden.add(b.dataset.k);localStorage.setItem(HKEY,JSON.stringify([...hidden]));render()});
document.getElementById('unhide').addEventListener('click',e=>{e.preventDefault();hidden.clear();localStorage.removeItem(HKEY);render()});
render();
</script></body></html>
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Find, Grouped};

    fn row(title: &str, artist: &str) -> Grouped {
        let f = Find {
            title: title.into(),
            artist: artist.into(),
            shazam_key: "k1".into(),
            when: 1_700_000_000,
            ..Default::default()
        };
        Grouped { find: f, first: 1_700_000_000, last: 1_700_003_600, count: 2 }
    }

    #[test]
    fn page_is_self_contained_and_carries_the_data() {
        let html = render(&[row("Resonance", "HOME")]);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Resonance"));
        assert!(html.contains("open.spotify.com"));
        assert!(html.contains("music.apple.com"));
        assert!(html.contains("youtube.com/results"));
        assert!(html.contains("shazam.com"));
        assert!(!html.contains("http://"), "no external assets over http");
        assert!(!html.contains("<link "), "no external stylesheets");
        assert!(!html.contains("<script src"), "no external scripts");
    }

    #[test]
    fn a_hostile_title_cannot_escape_the_embedded_json() {
        let html = render(&[row("</script><script>alert(1)</script>", "\"quoted\" & <b>")]);
        // The literal closing tag must never appear inside the data block.
        let data_start = html.find("id=\"data\"").unwrap();
        let data_end = html[data_start..].find("</script>").unwrap() + data_start;
        let block = &html[data_start..data_end];
        assert!(!block.contains("</script>"));
        assert!(block.contains("<\\/script>"));
    }

    /// Writes a sample page to target/eyeball/songs.html for a human (or a headless browser) to look
    /// at. Same pattern as the banner eyeball test; run with `--ignored`.
    #[test]
    #[ignore]
    fn eyeball_history_page() {
        let mut a = row("Resonance", "HOME");
        a.find.album = Some("Odyssey".into());
        a.find.cover_url = Some("https://is1-ssl.mzstatic.com/image/thumb/Music125/v4/7e/0c/8e/7e0c8e2b-1e2b-8f4e-7a3b-5d2f6d1c9a3a/cover.jpg/400x400bb.jpg".into());
        let mut b = row("Nightcall", "Kavinsky");
        b.find.shazam_key = "k2".into();
        b.find.album = Some("OutRun".into());
        b.count = 1;
        b.last = 1_690_000_000;
        let mut c = row("A Real Hero (feat. Electric Youth)", "College");
        c.find.shazam_key = "k3".into();
        c.count = 5;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("songs.html"), render(&[a, b, c])).unwrap();
    }

    #[test]
    fn empty_history_still_renders() {
        let html = render(&[]);
        assert!(html.contains("no songs yet"));
    }
}
