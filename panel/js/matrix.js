/* Champ select's ranked tables (1a bans, 1b blind pick, 1c best picks): candidates as rows, best
   first, and one column per thing they're scored on. Every row is its own grid on the same
   template (--cols), so the columns line up; the column under the pointer lights up. Columns can
   be grouped (AGAINST / WITH, YOUR LIKELY PICKS / TEAM HOVERS) under a coloured caption. */
(function(L){
  var el=L.el;
  var MINUS="−";

  /* signed number with a real minus sign; |v| under half the last digit prints unsigned */
  function fmt(v,d){
    var n=Number(v);if(!isFinite(n))return"-";
    d=d==null?1:d;var t=Math.abs(n).toFixed(d),z=Number(t)===0;
    return z?t:(n>0?"+":MINUS)+t;
  }
  function px(n){return"calc("+n+" * var(--px))"}

  /* box: the .mx element (cleared). o: {
       cols:[{w: width in mockup px or a CSS track, head: Element|string, cls, group, align, title}],
       groups:{name:{label, cls}},   caption over consecutive columns with that group
       rows:[{key, cells:[Element|null], cls, title}], one cell per column (null: no data, a dot)
       sel: key of the selected row, onSelect(key): rows become clickable,
       gap: column gap in mockup px (default 4), cls: extra class on the table }
     returns the table element (.rt) */
  function table(box,o){
    var t=el("div","rt"+(o.cls?" "+o.cls:""));
    /* the template: a spacer track between two different groups */
    var tracks=[],at=[],prev=null;
    o.cols.forEach(function(c,i){
      if(c.group&&prev&&c.group!==prev){tracks.push(px(14));}
      at.push(tracks.length+1);
      tracks.push(typeof c.w==="number"?px(c.w):c.w);
      prev=c.group||null;
    });
    t.style.setProperty("--cols",tracks.join(" "));
    t.style.setProperty("--cgap",px(o.gap==null?4:o.gap));

    /* header: group captions on the first line, column heads under them */
    var h=el("div","rt-h mr");
    var hasGroups=o.cols.some(function(c){return c.group});
    o.cols.forEach(function(c,i){
      if(c.group&&(i===0||o.cols[i-1].group!==c.group)){
        var n=1;while(o.cols[i+n]&&o.cols[i+n].group===c.group)n++;
        var g=(o.groups||{})[c.group]||{label:c.group};
        var cap=el("div","rt-g "+(g.cls||""),g.label);
        cap.style.gridColumn=at[i]+" / span "+(at[i+n-1]-at[i]+1);cap.style.gridRow="1";
        h.appendChild(cap);
      }
      var x=el("div","mc rt-ch"+(c.align?" "+c.align:"")+(c.cls?" "+c.cls:""));
      x.dataset.c=i;x.style.gridColumn=String(at[i]);x.style.gridRow=hasGroups?"2":"1";
      if(c.head!=null)x.appendChild(typeof c.head==="string"?el("span",null,c.head):c.head);
      if(c.title)x.title=c.title;
      h.appendChild(x);
    });
    if(!hasGroups)h.classList.add("flat");
    t.appendChild(h);

    var body=el("div","rt-b");
    o.rows.forEach(function(r,ri){
      var row=el("div","rt-r mr"+(r.cls?" "+r.cls:"")+(o.sel!=null&&r.key===o.sel?" sel":""));
      if(r.title)row.title=r.title;
      r.cells.forEach(function(x,i){
        var c=o.cols[i],d=el("div","mc"+(c.align?" "+c.align:"")+(c.cls?" "+c.cls:""));
        d.dataset.c=i;d.style.gridColumn=String(at[i]);
        d.appendChild(x||el("span","none","·"));
        row.appendChild(d);
      });
      if(o.onSelect){
        row.classList.add("pickable");row.tabIndex=0;row.setAttribute("role","button");
        row.setAttribute("aria-pressed",o.sel===r.key?"true":"false");
        row.addEventListener("click",function(){o.onSelect(r.key)});
        row.addEventListener("keydown",function(e){if(e.key==="Enter"||e.key===" "){e.preventDefault();o.onSelect(r.key)}});
      }
      body.appendChild(row);
    });
    t.appendChild(body);
    t.onmouseover=function(e){light(t,e.target.closest&&e.target.closest(".mc[data-c]"))};
    t.onmouseleave=function(){light(t,null)};
    box.appendChild(t);
    return t;
  }
  function light(t,c){
    var k=c&&!c.classList.contains("nolight")?c.dataset.c:null;
    if(k===t._lit)return;
    t._lit=k;
    t.querySelectorAll(".mc.lit").forEach(function(e){e.classList.remove("lit")});
    if(k!=null)t.querySelectorAll('.mc[data-c="'+k+'"]').forEach(function(e){e.classList.add("lit")});
  }

  /* tint alpha for |v| on a column whose biggest value is max (as the mockups) */
  function alpha(v,max){return Math.min(0.12+Math.abs(v)/(max||1)*0.5,0.62)}
  /* a heat cell: the number on a green / coral tint as strong as |v| is big (white on strong tints) */
  function heat(v,max,title,txt){
    var n=Number(v)||0,b=el("span","mval heat");
    var s=txt!=null?txt:fmt(n);
    if(Math.abs(n)<0.05){b.classList.add("zero");b.textContent=txt!=null?txt:"0.0";}
    else{
      var a=alpha(n,max);
      b.textContent=s;
      b.style.background="color-mix(in srgb, var("+(n>0?"--pos":"--neg")+") "+Math.round(a*100)+"%, transparent)";
      b.classList.add(a>0.3?"strong":n>0?"pos":"neg");
    }
    if(title)b.title=title;
    return b;
  }
  /* a plain value cell on a neutral chip */
  function plain(txt,title){var b=el("span","mval heat flat",txt);if(title)b.title=title;return b}
  /* biggest |value| of a list (at least lo), for scaling tints and bars */
  function maxAbs(list,lo){return list.reduce(function(m,v){v=Math.abs(Number(v)||0);return v>m?v:m},lo==null?0.5:lo)}

  /* rank number */
  function rank(i){return el("span","rt-rank",String(i))}
  /* candidate: portrait, name, optional sub line */
  function who(name,icon,sub){
    var d=el("span","rt-who");
    d.appendChild(L.icon(name,icon,"rt-pt"));
    var t=el("span","rt-nm");t.appendChild(el("span","n",name));
    if(sub)t.appendChild(el("span","s",sub));
    d.appendChild(t);
    return d;
  }
  /* score: big number over a thin bar (length = v / max) */
  function score(txt,v,max,cls){
    var d=el("span","rt-score "+(cls||""));
    d.appendChild(el("b",null,txt));
    var bar=el("span","bar"),f=el("i");
    f.style.width=Math.max(0,Math.min(100,Math.abs(Number(v)||0)/(max||1)*100)).toFixed(0)+"%";
    bar.appendChild(f);d.appendChild(bar);
    return d;
  }
  /* column head: a portrait (dimmed when only hovered) */
  function headIcon(name,icon,dim,title){
    var w=el("span","rt-hi"+(dim?" dim":""));w.appendChild(L.icon(name,icon,"rt-hpt"));
    if(title)w.title=title;
    return w;
  }
  /* diverging bar around a centre line: v / max of each half */
  function divbar(v,max){
    var n=Number(v)||0,w=Math.min(Math.abs(n)/(max||1)*100,100).toFixed(0)+"%";
    var d=el("span","dbar2"),l=el("span","h l"),r=el("span","h r"),li=el("i"),ri=el("i");
    if(n<0)li.style.width=w;else ri.style.width=w;
    l.appendChild(li);r.appendChild(ri);
    d.appendChild(l);d.appendChild(el("span","c"));d.appendChild(r);
    return d;
  }

  L.mx={table:table,heat:heat,plain:plain,maxAbs:maxAbs,rank:rank,who:who,score:score,headIcon:headIcon,
    divbar:divbar,fmt:fmt,px:px};
})(window.LPH);
