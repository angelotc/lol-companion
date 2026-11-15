/* Client screen section, after lock-in (1d): champion and rune page on the left (laid out like the
   League client's rune page), start / core / item slots on the right, skill order underneath. Every
   option shows its Δ vs the enemies it is measured against (hover for each enemy's share). */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,icon=L.icon,fmt=L.fmt,cls=L.cls,clear=L.clear;
  var SIG=0.3;  /* matchup deltas smaller than this are shown as neutral */
  /* rune tree accents, as in the client */
  var TREE={8000:"#C8AA6E",8100:"#D44242",8200:"#9FAAFC",8300:"#49AAB9",8400:"#A1D586"};
  var pageIdx=0,lastChamp=null,last=null;  /* the rune page shown (0 most picked), kept across updates */

  function k(n){n=Number(n)||0;return n>=1000?(n/1000).toFixed(n>=10000?0:1)+"k":String(n)}
  function has(o){return !!(o&&o.vs&&o.vs.length)}
  /* Δ text and class: blank without matchup data, ±0 when too small to matter */
  function dTxt(o){if(!has(o))return"";var m=Number(o.matchup)||0;return Math.abs(m)<SIG?"±0":fmt(m)}
  function dCls(o){if(!has(o))return"";var m=Number(o.matchup)||0;return Math.abs(m)<SIG?"z":cls(m)}
  function vsTip(o){
    var t=el("div","vsd");
    if(!has(o)){t.appendChild(el("div",null,"No matchup data (too few games)"));return t}
    o.vs.forEach(function(v){
      var r=el("div");r.appendChild(el("span",null,"vs"));r.appendChild(icon(v.name,v.icon));
      r.appendChild(el("span",null,v.name+" ("+lane(v.lane)+")"));
      r.appendChild(el("b",cls(v.delta),fmt(v.delta)));r.appendChild(el("span",null,"· "+k(v.games)+" games"));
      t.appendChild(r);
    });
    return t;
  }
  function names(o){return (o.parts||[]).map(function(p){return p.name}).join(" › ")}
  function optLine(o){return names(o)+": "+o.wr+"% win · "+o.pr+"% picked · "+k(o.games)+" games"+(has(o)?" · Δ "+dTxt(o):"")}
  function icons(o,sep,cl){
    var its=el("span",cl||"its");
    (o.parts||[]).forEach(function(p,i){
      if(i&&sep)its.appendChild(el("span","arrow","›"));
      var ic=icon(p.name,p.icon);ic.title=p.name;its.appendChild(ic);
    });
    return its;
  }
  function stat(o,extra){
    var s=el("span","bst");s.appendChild(el("b",null,o.wr+"%"));
    s.appendChild(document.createTextNode(" win · "+o.pr+"% picked"+(extra?" · "+k(o.games)+" games":"")));
    return s;
  }

  /* ---- rune page ---- */
  function runeTitle(r){
    return r.name+(r.wr!=null?"\n"+r.wr+"% win · "+r.pr+"% picked · "+k(r.games)+" games":"")+
      (has(r)?"\nΔ vs lane "+dTxt(r)+(r.vs||[]).map(function(v){return "\n  vs "+v.name+": "+fmt(v.delta)+" ("+k(v.games)+" games)"}).join(""):"");
  }
  function rune(r,kind){
    var d=el("div","rn "+kind);
    var ic=icon(r.name,r.icon);d.appendChild(ic);
    d.appendChild(el("span","dl "+dCls(r),dTxt(r)));
    d.title=runeTitle(r);
    return d;
  }
  function treeHead(st){
    var h=el("div","rth");
    if(st){h.appendChild(icon(st.name,st.icon));h.appendChild(el("span",null,st.name))}
    return h;
  }
  /* one row of a tree as the client draws it: every option, the page's pick lit, the rest dimmed;
     the pick's Δ at the row's end. keys: the keystone row (bigger, pick % under each) */
  function treeRow(row,keys,winId){
    var r=el("div","rrow"+(keys?" keys":"")),ics=el("div","rro"),pick=null;
    row.forEach(function(x){
      var alt=!x.chosen&&winId!=null&&Number(x.id)===winId;
      var e=icon(x.name,x.icon,(x.chosen?"on":"off")+(alt?" alt":""));
      e.title=runeTitle(x)+(alt?"\nHighest win rate shard in this row":"");
      if(x.chosen)pick=x;
      if(keys){var w=el("span","rk"+(x.chosen?" on":""));w.title=e.title;w.appendChild(e);
        w.appendChild(el("span","rkp",x.pr!=null?x.pr+"%":""));ics.appendChild(w)}
      else ics.appendChild(e);
    });
    r.appendChild(ics);
    r.appendChild(el("span","dl "+(pick?dCls(pick):""),pick?dTxt(pick):""));
    return r;
  }
  function shardRows(rows,chosen,winIds){
    var box=el("div","rshards");
    /* rows from the payload; without them, the chosen shard of each row only */
    (rows&&rows.length?rows:chosen.map(function(r){var c={chosen:true};for(var q in r)c[q]=r[q];return [c]})).forEach(function(row,i){
      box.appendChild(treeRow(row,false,winIds?winIds[i]:null));
    });
    return box;
  }
  function tc(node,st){node.style.setProperty("--tc",TREE[st&&st.id]||"var(--gold)")}
  function runePage(p){
    var st=p.styles||[],t=p.tree||{};
    var win=(p.win_shards||[]).map(function(r){return Number(r.id)});
    var full=t.primary&&t.primary.length&&t.secondary&&t.secondary.length;
    var pg=el("div","rpage"+(full?" full":""));
    var prim=el("div","rtree prim"),sec=el("div","rtree sec");
    tc(prim,st[0]);tc(sec,st[1]);
    prim.appendChild(treeHead(st[0]));sec.appendChild(treeHead(st[1]));
    if(full){
      /* the whole tree, row by row, as in the client */
      t.primary.forEach(function(row,i){prim.appendChild(treeRow(row,i===0))});
      t.secondary.forEach(function(row){sec.appendChild(treeRow(row,false))});
    }else{
      /* tree layout unknown: the chosen runes only */
      var path=el("div","rpath");
      (p.primary||[]).forEach(function(r,i){path.appendChild(rune(r,i===0?"key":"minor"))});
      prim.appendChild(path);
      var path2=el("div","rpath");
      (p.secondary||[]).forEach(function(r){path2.appendChild(rune(r,"minor"))});
      sec.appendChild(path2);
    }
    sec.appendChild(shardRows(t.shards,p.shards||[],win.length?win:null));
    pg.appendChild(prim);pg.appendChild(sec);
    return pg;
  }
  function runeCard(d){
    var card=el("div","bcard brunes");
    var pages=d.pages||[];
    if(pageIdx>=pages.length)pageIdx=0;
    var p=pages[pageIdx];
    var h=el("div","bch");
    var hl=el("div","bchl");
    if(pages.length>1){
      hl.title="Rune pages: most picked, highest win rate";
      var pill=el("div","pill");
      pages.forEach(function(q,i){
        var b=el("button",i===pageIdx?"on":"",i===0?"Most picked":"Highest win rate");b.title=q.label+": "+q.wr+"% win · "+k(q.games)+" games";
        b.onclick=function(){if(pageIdx!==i){pageIdx=i;if(last)render(last.d,last.s)}};
        pill.appendChild(b);
      });
      hl.appendChild(pill);
    }else if(p){hl.appendChild(el("span","cap","Runes"));var pl=el("span","bchp",p.label);pl.title=p.label;hl.appendChild(pl)}
    h.appendChild(hl);
    if(p)h.appendChild(el("span","bchr",p.wr+"% · "+k(p.games)+" games"+(p.small?" · few":"")));
    card.appendChild(h);
    if(p)card.appendChild(runePage(p));
    /* the full keystone row already shows every keystone's pick %: the separate row only without it */
    var fullTree=p&&p.tree&&p.tree.primary&&p.tree.primary.length;
    if(!fullTree&&d.keystones&&d.keystones.length>1){
      var ks=el("div","rkeys");ks.appendChild(el("span","cap","Keystones"));
      d.keystones.forEach(function(r){
        var e=el("span","rkk");e.appendChild(icon(r.name,r.icon));
        e.appendChild(el("span","rkp",r.pr+"%"));
        e.appendChild(el("span","dl "+dCls(r),dTxt(r)));
        e.title=runeTitle(r);ks.appendChild(e);
      });
      card.appendChild(ks);
    }
    if(d.spells&&d.spells.length){
      var sp=el("div","bspells");sp.appendChild(el("span","cap","Summoner spells"));
      d.spells.forEach(function(o){
        var r=el("div","sprow"+(o.best?" best":""));
        r.appendChild(icons(o,false));
        r.appendChild(el("span","spn",(o.parts||[]).map(function(x){return x.name}).join(" + ")));
        var w=el("span","spw");w.appendChild(el("b",null,o.wr+"%"));w.appendChild(document.createTextNode(" · "+o.pr+"%"));r.appendChild(w);
        r.appendChild(el("span","dl "+dCls(o),dTxt(o)));
        r.title=optLine(o).replace(/ › /g," + ")+(o.best?"\nBest vs lane":"");
        r.appendChild(vsTip(o));
        sp.appendChild(r);
      });
      card.appendChild(sp);
    }
    return card;
  }

  /* ---- items ---- */
  function itemCard(o){
    var c=el("div","icard"+(o.best?" best":""));
    var p=(o.parts||[])[0]||{};
    c.appendChild(icon(p.name,p.icon,"iic"));
    var m=el("div","icm");
    var nm=el("span","icn",names(o));nm.title=optLine(o);m.appendChild(nm);
    var l=el("span","icl");l.appendChild(el("b",null,o.wr+"%"));
    var bar=el("span","pbar"),f=el("span");f.style.width=Math.max(0,Math.min(100,Number(o.pr)||0))+"%";bar.appendChild(f);
    l.appendChild(bar);l.appendChild(el("span",null,o.pr+"%"));
    m.appendChild(l);c.appendChild(m);
    c.appendChild(el("span","icd "+dCls(o),dTxt(o)));
    if(o.best)c.appendChild(el("span","tag","BEST VS LANE"));
    c.appendChild(vsTip(o));
    return c;
  }
  /* the options that fit: the most picked, always keeping the best vs lane */
  function capped(opts,n){
    if(opts.length<=n)return opts;
    var best=opts.filter(function(o){return o.best})[0];
    var top=opts.slice(0,n);
    if(best&&top.indexOf(best)<0)top=opts.slice(0,n-1).concat([best]);
    return top;
  }
  function slotCol(sec,n,up){
    var col=el("div","bslot"+(up?" up":""));
    var h=el("div","bsh");h.appendChild(el("span","cap",sec.title));
    var shown=capped(sec.options||[],n),rest=(sec.options||[]).filter(function(o){return shown.indexOf(o)<0});
    var r=el("span","bshr");
    if(rest.length){var more=el("span","more","+"+rest.length+" more");more.title=rest.map(optLine).join("\n");r.appendChild(more)}
    r.appendChild(document.createTextNode("Δ VS LANE"));
    h.appendChild(r);col.appendChild(h);
    shown.forEach(function(o){col.appendChild(itemCard(o))});
    return col;
  }
  function topStrip(d,secs){
    var t=el("div","btop");
    var start=(secs.start&&secs.start.options||[])[0];
    if(start){
      var a=el("div","bcard bstrip bstart");a.appendChild(el("span","cap","Start"));
      a.appendChild(icons(start,false));a.appendChild(stat(start));
      var sd=dTxt(start);if(sd)a.appendChild(el("span","dl "+dCls(start),sd));
      a.appendChild(vsTip(start));
      t.appendChild(a);
    }
    var cores=secs.core&&secs.core.options||[];
    var b=el("div","bcard bstrip bcore");
    if(cores.length){
      b.appendChild(el("span","cap","Most built core"));
      b.appendChild(icons(cores[0],true));b.appendChild(stat(cores[0],true));
      if(cores.length>1)b.title="Other core paths:\n"+cores.slice(1).map(optLine).join("\n");
    }
    if(d.skills&&d.skills.length){
      var mx=el("span","bmax");mx.appendChild(document.createTextNode("Max "));
      mx.appendChild(el("b",null,d.skills[0].key.split("").join(" › ")));
      mx.title=d.skills.map(function(x){return "Max "+x.key.split("").join(" › ")+": "+x.wr+"% win · "+x.pr+"% picked · "+k(x.games)+" games"}).join("\n");
      b.appendChild(mx);
    }
    if(cores.length||d.skills&&d.skills.length)t.appendChild(b);
    return t;
  }
  function skillStrip(d){
    var s=el("div","bskills");
    s.appendChild(el("span","cap bsl","Skills"));
    var ord=el("div","bsk"),max=d.skills&&d.skills[0]?d.skills[0].key.charAt(0):"";
    d.skill_order.split("").forEach(function(c,i){
      var x=el("span","sk"+(c==="R"?" R":c===max?" max":""));
      x.appendChild(el("span","sq",c));x.appendChild(el("span","lv",String(i+1)));ord.appendChild(x);
    });
    s.appendChild(ord);
    return s;
  }
  function explain(){
    var n=el("div","bexp","Δ = how much better an option does against these enemies than usual, in win-rate points. Options with too few games show no Δ.");
    n.title="Δ compares an option's win rate against the enemies shown (lane opponents full weight; junglers and roamers less) with its usual win rate, "+
      "shrunk by games (hover an option for each opponent). Later items show higher win rates because longer games are more often won: compare options within a slot.";
    return n;
  }

  function render(d,s){
    last={d:d,s:s};
    var head=$("bHead"),body=$("bBody");clear(head);clear(body);
    var champ=d.champ?d.champ.name:"";
    if(champ!==lastChamp){pageIdx=0;lastChamp=champ}

    var left=el("div","bleft");
    var ch=el("div","bchamp");
    if(d.champ)ch.appendChild(icon(d.champ.name,d.champ.icon,"bport"));
    var t=el("div","bct");
    t.appendChild(el("span","bcn",champ));
    if(d.games)t.appendChild(el("span","bcs",d.wr+"% win · "+k(d.games)+" games"));
    var vs=el("span","bcv");
    if(d.opponents&&d.opponents.length){
      vs.appendChild(document.createTextNode("Δ measured vs"));
      d.opponents.forEach(function(o){var i=icon(o.name,o.icon);i.title=o.name+" ("+lane(o.lane)+"): "+k(o.games)+" games, "+o.wr+"% win";vs.appendChild(i)});
    }else if(!d.empty)vs.appendChild(document.createTextNode("Δ shows once your lane opponent locks in"));
    t.appendChild(vs);ch.appendChild(t);left.appendChild(ch);
    body.appendChild(left);
    if(d.empty){var e=el("div","bright");e.appendChild(el("div","bnote",d.empty));body.appendChild(e);return}
    left.appendChild(runeCard(d));

    var right=el("div","bright"),secs={},slots=[];
    (d.items||[]).forEach(function(sec){secs[sec.key]=sec;if(sec.key!=="start"&&sec.key!=="core")slots.push(sec)});
    right.appendChild(topStrip(d,secs));
    var cols=Math.min(3,Math.max(1,slots.length)),rows=Math.ceil(slots.length/cols);
    var grid=el("div","bslots");grid.style.setProperty("--cols",cols);
    var n=rows>1?3:6;  /* two rows of slots fit three options each */
    slots.forEach(function(sec,i){grid.appendChild(slotCol(sec,n,rows>1&&i>=cols))});
    right.appendChild(grid);
    var foot=el("div","bfoot");
    if(d.skill_order)foot.appendChild(skillStrip(d));
    foot.appendChild(explain());
    right.appendChild(foot);
    body.appendChild(right);
  }
  L.section("build",{render:render});
})(window.LPH);
