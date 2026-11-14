/* Champ select's comparison table: candidates (picks, bans) as columns left to right by rank, and
   below them one row per thing they're scored on, so every candidate's numbers line up. One CSS
   grid; rows are display:contents wrappers (hover lights the row), and the column under the
   pointer lights up too. */
(function(L){
  var el=L.el;

  /* box: the .mx element. cols: [{head:Element, cls}]; rows: [{head:Element, cls, cells:[Element|null]}],
     one cell per column (null: no data, drawn as a dot). */
  function matrix(box,cols,rows){
    L.clear(box);
    box.style.setProperty("--n",cols.length);
    var hr=el("div","mr mh");hr.appendChild(el("div","mc rh"));
    cols.forEach(function(c,i){hr.appendChild(cell(c.head,i,"ch "+(c.cls||"")))});
    box.appendChild(hr);
    rows.forEach(function(r){
      var row=el("div","mr"+(r.cls?" "+r.cls:""));
      var h=el("div","mc rh");h.appendChild(r.head);row.appendChild(h);
      r.cells.forEach(function(x,i){row.appendChild(cell(x||el("span","none","·"),i,cols[i].cls||""))});
      box.appendChild(row);
    });
    box.onmouseover=function(e){light(box,e.target.closest&&e.target.closest(".mc[data-c]"))};
    box.onmouseleave=function(){light(box,null)};
  }
  function cell(content,i,cls){
    var d=el("div","mc"+(cls?" "+cls:""));d.dataset.c=i;d.appendChild(content);return d;
  }
  function light(box,c){
    var k=c?c.dataset.c:null;
    if(k===box._lit)return;
    box._lit=k;
    box.querySelectorAll(".mc.lit").forEach(function(e){e.classList.remove("lit")});
    if(k!=null)box.querySelectorAll('.mc[data-c="'+k+'"]').forEach(function(e){e.classList.add("lit")});
  }

  /* a value cell: the number (colored by sign, tinted by how much it counts), and optionally a
     smaller second number (what it adds to the score) */
  function val(v,weight,small,title){
    var f=el("span","mval");
    var b=el("b",L.cls(v),L.fmt(v));
    var a=Math.min(1,Math.abs(Number(weight)||0)/3)*0.28;
    if(a>0.02)b.style.background=(Number(weight)>=0?"rgba(95,255,135,":"rgba(255,95,95,")+a.toFixed(3)+")";
    f.appendChild(b);
    if(small!=null)f.appendChild(el("i",null,small));
    if(title)f.title=title;
    return f;
  }

  /* a column header: rank, portrait, score, name */
  function head(rank,name,icon,score,scoreCls,sub){
    var d=el("div","pk");
    var t=el("div","pk-t");
    t.appendChild(L.icon(name,icon,"big"));
    var r=el("div","pk-r");r.appendChild(el("span","rank",String(rank)));
    r.appendChild(el("span","score "+(scoreCls||""),score));
    t.appendChild(r);d.appendChild(t);
    d.appendChild(el("div","name",name));
    if(sub)d.appendChild(el("div","pk-s",sub));
    return d;
  }

  /* a row header: a kind label, then a champion (icon + name + lane) or just text */
  function label(kind,name,icon,lane){
    var d=el("span","rl");
    if(kind)d.appendChild(el("span","k",kind));
    if(icon!==undefined)d.appendChild(L.icon(name,icon));
    d.appendChild(el("span","n",name));
    if(lane)d.appendChild(el("span","ln",L.lane(lane)));
    return d;
  }

  L.matrix=matrix;L.mxVal=val;L.mxHead=head;L.mxLabel=label;
})(window.LPH);
