/* Client screen section (1b blind pick, 1c best picks).
   1c: the ranked picks as rows (rank, portrait, score), then one column per term of the score:
   strength (Base), each known enemy (AGAINST) and teammate (WITH), as the points it adds; beside
   it, the selected pick's breakdown (click a row; the top pick by default).
   1b: the blind-safe picks as rows: score, strength, team fit, the range from "if countered" to
   "average matchup", blind risk, worst counters and how much matchup data there is. */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,fmt=L.mx.fmt,clear=L.clear,mx=L.mx;
  var KEY_RECS="Each cell is the win-rate points that term adds to the score: strength, each matchup and duo, and your own games and mastery. Click a pick for the raw numbers.";
  var KEY_BLIND="Enemy {role} not picked yet. Score = strength + team fit + blind risk. “If countered” is the average of the worst matchups that make up 15% of enemy picks.";
  /* the line under the table (key, locked-in badge): index.html's, moved under the table each render */
  var keyRow=$("recKey").parentNode,keyEl=$("recKey"),lockedEl=$("locked");
  var sel=null;   // selected pick (1c), by champion name: survives new data
  var last=null;  // last render's arguments, to redraw on a click

  function pts(n){n=Number(n)||0;return n>=1e6?(n/1e6).toFixed(1)+"M":n>=1e3?Math.round(n/1e3)+"k":String(n)}
  function laneIdx(l){var i=L.LANE_ORDER.indexOf(l);return i<0?9:i}
  function key(c){return c.kind+":"+c.name}

  function renderLocked(l){
    clear(lockedEl);lockedEl.classList.toggle("hidden",!l);
    if(!l)return;
    lockedEl.appendChild(L.icon(l.name,l.icon));
    lockedEl.appendChild(el("span",null,"Locked in: "+l.name));
    lockedEl.appendChild(el("span","sub2","picks as they were at lock-in"));
  }

  /* the AGAINST / WITH columns: every known enemy and teammate (and any term naming someone else),
     each group in lane order */
  function termCols(d,recs){
    var seen={},list=[];
    function add(kind,name,icon,ln,tent){
      var k=kind+":"+name;if(seen[k]){if(tent)seen[k].tentative=true;return}
      seen[k]={kind:kind,name:name,icon:icon,lane:ln,tentative:!!tent};list.push(seen[k]);
    }
    (d.enemies||[]).forEach(function(e){add("vs",e.name,e.icon,e.lane)});
    (d.allies||[]).forEach(function(a){if(!a.is_me)add("with",a.name,a.icon,a.lane,a.tentative)});
    recs.forEach(function(r){(r.contributions||[]).forEach(function(c){
      if(c.kind==="vs"||c.kind==="with")add(c.kind,c.name,c.icon,c.lane,c.tentative);
    })});
    list.sort(function(a,b){return((a.kind==="vs"?0:1)-(b.kind==="vs"?0:1))||(laneIdx(a.lane)-laneIdx(b.lane))});
    return list;
  }
  function term(r,k){return(r.contributions||[]).filter(function(x){return x.kind+":"+x.name===k})[0]}
  function termTip(r,c){
    var t=c.kind==="you"?"Your games on "+r.name+": "+fmt(c.delta)+" win rate vs usual in "+c.games+" games":
      c.kind==="mastery"?pts(c.delta)+" mastery points on "+r.name:
      r.name+" "+(c.kind==="with"?"with ":"vs ")+c.name+" ("+lane(c.lane)+"): "+fmt(c.delta)+" win rate in "+c.games+" games";
    return t+" → "+fmt(c.weighted)+" to the score"+(c.tentative?" (still hovering: counts less)":"");
  }
  function baseTip(r){return r.name+": "+fmt(r.base,2)+" win rate vs an average pick in this role, before this draft"}

  /* ---------- 1c ---------- */
  function renderRecs(box,d){
    var recs=d.recs||[],locked=d.locked,tcols=termCols(d,recs);
    var names=recs.map(function(r){return r.name});
    if(sel==null||names.indexOf(sel)<0)sel=locked&&names.indexOf(locked.name)>=0?locked.name:(names[0]||null);
    /* your own games and mastery, as one column: with it a row's cells add up to its score */
    function own(r){return(r.contributions||[]).filter(function(c){return c.kind==="you"||c.kind==="mastery"})}
    var vals=[];recs.forEach(function(r){vals.push(r.base);(r.contributions||[]).forEach(function(c){if(c.kind==="vs"||c.kind==="with")vals.push(c.weighted)});
      vals.push(own(r).reduce(function(s,c){return s+c.weighted},0))});
    var max=mx.maxAbs(vals,1),top=mx.maxAbs(recs.map(function(r){return r.score}),0.01);

    var tw=tcols.length>=9?44:48;  // narrower term columns on a full draft, so the name still fits
    var cols=[{w:28,head:""},{w:"minmax(0,1fr)",head:"Pick"},{w:76,head:"Score",align:"r"},
      {w:56,head:"Base",align:"c",title:"Strength: win rate vs an average pick in this role"}];
    tcols.forEach(function(c){
      cols.push({w:tw,align:"c",group:c.kind,cls:c.tentative?"tent":"",
        head:mx.headIcon(c.name,c.icon,c.tentative,c.name+", "+lane(c.lane)+(c.tentative?" (hovering)":""))});
    });
    cols.push({w:tw,align:"c",group:"you",head:"",title:"Your own games on the champion, and your mastery"});
    var rows=recs.map(function(r,i){
      var cells=[mx.rank(i+1),mx.who(r.name,r.icon),
        mx.score(fmt(r.score,2),r.score,top,r.score<0?"neg":"pos"),
        mx.heat(r.base,max,baseTip(r))];
      tcols.forEach(function(c){
        var x=term(r,key(c));
        cells.push(x?mx.heat(x.weighted,max,termTip(r,x)):null);
      });
      var o=own(r);
      cells.push(o.length?mx.heat(o.reduce(function(s,c){return s+c.weighted},0),max,
        o.map(function(c){return termTip(r,c)}).join("\n")):null);
      return{key:r.name,cells:cells,cls:(locked&&locked.name===r.name?"mine":"")};
    });

    var main=el("div","mx-main");
    mx.table(main,{cols:cols,rows:rows,sel:sel,
      groups:{vs:{label:"Against",cls:"g-vs"},"with":{label:"With",cls:"g-with"},you:{label:"You",cls:"g-pick"}},
      onSelect:function(k){sel=k;draw()}});
    main.appendChild(keyRow);
    box.appendChild(main);
    var r=recs.filter(function(x){return x.name===sel})[0];
    if(r)box.appendChild(detail(r,names.indexOf(r.name),locked,tcols));
  }

  /* the selected pick: header, best / worst term, and every term of its score */
  function detail(r,i,locked,tcols){
    var side=el("div","mx-side");
    var hd=el("div","dt-h");
    hd.appendChild(L.icon(r.name,r.icon,"dt-pt"));
    var nm=el("div","dt-nm");
    var isLocked=locked&&locked.name===r.name;
    nm.appendChild(el("span","dt-k",isLocked?"LOCKED IN":i===0?"TOP PICK":"PICK #"+(i+1)));
    nm.appendChild(el("span","dt-n",r.name));
    hd.appendChild(nm);
    var sc=el("div","dt-sc");
    sc.appendChild(el("div","v "+(r.score<0?"neg":"pos"),fmt(r.score,2)));
    sc.appendChild(el("div","u","win-rate points"));
    hd.appendChild(sc);
    side.appendChild(hd);

    /* the terms in the table's column order, then your own games and mastery */
    var order={};tcols.forEach(function(c,j){order[key(c)]=j});
    var cs=(r.contributions||[]).slice().sort(function(a,b){
      var x=order[key(a)],y=order[key(b)];
      return(x==null?99+(a.kind==="mastery"?1:0):x)-(y==null?99+(b.kind==="mastery"?1:0):y);
    });
    var pair=cs.filter(function(c){return c.kind==="vs"||c.kind==="with"});
    var best=null,worst=null;
    pair.forEach(function(c){if(!best||c.weighted>best.weighted)best=c;if(!worst||c.weighted<worst.weighted)worst=c});
    var tiles=el("div","dt-tiles");
    function tile(cap,c,none){
      var t=el("div","dt-tile");t.appendChild(el("div","cap",cap));
      var v=el("div","v");
      if(c){v.appendChild(L.icon(c.name,c.icon,"dt-ti"));
        v.appendChild(el("span",null,(c.kind==="vs"?"vs ":"w/ ")+c.name));
        v.appendChild(el("b",c.weighted<0?"neg":"pos",fmt(c.weighted)));
        t.title=termTip(r,c);}
      else v.appendChild(el("span","none",none));
      t.appendChild(v);return t;
    }
    tiles.appendChild(tile("Best into",best&&best.weighted>0?best:null,"No matchup or duo helps"));
    tiles.appendChild(tile("Watch out for",worst&&worst.weighted<0?worst:null,"No matchup or duo hurts"));
    side.appendChild(tiles);

    var th=el("div","dt-row dt-th");
    ["Why this score","Δ win %","Adds",""].forEach(function(t,j){th.appendChild(el("span","cap"+(j===1?" r":j===2?" c":""),t))});
    side.appendChild(th);
    var terms=[{tag:"",name:"Base strength",raw:fmt(r.base),v:r.base,tip:baseTip(r)}];
    cs.forEach(function(c){
      terms.push(c.kind==="you"?{tag:"",name:"Your games",raw:fmt(c.delta),v:c.weighted,tip:termTip(r,c)}:
        c.kind==="mastery"?{tag:"",name:"Mastery",raw:pts(c.delta)+" pts",v:c.weighted,tip:termTip(r,c)}:
        {tag:c.kind==="vs"?"vs":"w/",kind:c.kind,name:c.name,icon:c.icon,raw:fmt(c.delta),v:c.weighted,tip:termTip(r,c),tent:c.tentative});
    });
    var bmax=mx.maxAbs(terms.map(function(t){return t.v}),1);
    var list=el("div","dt-list");
    terms.forEach(function(t){
      var row=el("div","dt-row"+(t.tent?" tent":""));row.title=t.tip;
      var lb=el("span","lb");
      if(t.icon!==undefined)lb.appendChild(L.icon(t.name,t.icon,"dt-li"));
      if(t.tag)lb.appendChild(el("span","tag "+(t.kind==="vs"?"vs":"w"),t.tag));
      lb.appendChild(el("span","n",t.name));
      row.appendChild(lb);
      row.appendChild(el("span","raw",t.raw));
      row.appendChild(mx.divbar(t.v,bmax));
      var z=Math.abs(t.v)<0.05;
      row.appendChild(el("span","add "+(z?"zero":t.v>0?"pos":"neg"),z?"0.0":fmt(t.v)));
      list.appendChild(row);
    });
    side.appendChild(list);
    return side;
  }

  /* ---------- 1b ---------- */
  function renderBlind(box,list){
    var strength=function(r){return r.score-r.value-r.synergy};
    var mS=mx.maxAbs(list.map(strength)),mF=mx.maxAbs(list.map(function(r){return r.synergy})),
        mR=mx.maxAbs(list.map(function(r){return r.value}));
    /* the shared axis: from the lowest "if countered" to the highest average matchup, 0 included */
    var lo=Math.min(0,Math.min.apply(null,list.map(function(r){return Math.min(r.tail,r.mean)}).concat([0])));
    var hi=Math.max(0,Math.max.apply(null,list.map(function(r){return Math.max(r.tail,r.mean)}).concat([0])));
    lo=Math.floor(lo);hi=Math.max(0.5,Math.ceil(hi*2)/2);if(hi-lo<1.5)lo=hi-1.5;
    var x=function(v){return((v-lo)/(hi-lo)*100).toFixed(1)+"%"};
    var axisHead=el("div","bl-ah");
    var lg=el("div","bl-lg");
    var l1=el("span");l1.appendChild(el("i","dot cnt"));l1.appendChild(document.createTextNode("if countered"));
    var l2=el("span");l2.appendChild(document.createTextNode("average matchup"));l2.appendChild(el("i","dot avg"));
    lg.appendChild(l1);lg.appendChild(l2);axisHead.appendChild(lg);
    var ticks=el("div","bl-ticks");
    for(var t=Math.ceil(lo);t<=Math.floor(hi);t++){var s=el("span",null,t===0?"0":fmt(t,0));s.style.left=x(t);ticks.appendChild(s)}
    axisHead.appendChild(ticks);

    var cols=[{w:28,head:""},{w:180,head:"Pick"},{w:90,head:"Score",align:"r"},{w:76,head:"Strength",align:"c"},
      {w:76,head:"Team fit",align:"c"},{w:"minmax(0,1fr)",head:axisHead,cls:"nolight"},{w:84,head:"Blind risk",align:"c"},
      {w:120,head:"Worst counters"},{w:84,head:"Matchup data",align:"r"}];
    var rows=list.map(function(r,i){
      var tr=el("div","bl-tr");
      tr.appendChild(el("span","ln"));
      var z=el("span","zero");z.style.left=x(0);tr.appendChild(z);
      var a=Math.min(r.tail,r.mean),b=Math.max(r.tail,r.mean);
      var seg=el("span","seg");seg.style.left=x(a);seg.style.width=((b-a)/(hi-lo)*100).toFixed(1)+"%";tr.appendChild(seg);
      var dc=el("span","dot cnt");dc.style.left=x(r.tail);tr.appendChild(dc);
      var da=el("span","dot avg");da.style.left=x(r.mean);tr.appendChild(da);
      var tc=el("span","lc",fmt(r.tail));tc.style.left=x(r.tail);tr.appendChild(tc);
      var ta=el("span","la",fmt(r.mean));ta.style.left=x(r.mean);tr.appendChild(ta);
      tr.title=r.name+": "+fmt(r.mean)+" vs a random enemy laner (pick-rate weighted), "+fmt(r.tail)+" vs its worst matchups (15% of enemy picks)";
      var w=el("span","bl-wc");
      (r.counters||[]).forEach(function(c){var ic=L.icon(c.name,c.icon,"bl-ci");ic.title=c.name+" "+fmt(c.delta)+" ("+c.games+" games)";w.appendChild(ic)});
      var thin=r.coverage<50;
      var cov=el("span","bl-cov"+(thin?" thin":""));
      var cb=el("span","bar"),ci=el("i");ci.style.width=Math.max(0,Math.min(100,r.coverage))+"%";cb.appendChild(ci);
      cov.appendChild(cb);cov.appendChild(el("span",null,r.coverage+"%"));
      cov.title=thin?"Few games at this tier: the counter risk is partly a typical value":"Share of enemy picks with matchup data";
      var st=strength(r);
      return{key:r.name,cls:i===0?"first":"",cells:[mx.rank(i+1),mx.who(r.name,r.icon),
        mx.score(fmt(r.score,2),0,1,r.score<0?"neg nobar":"pos nobar"),
        mx.heat(st,mS,r.name+": overall strength in this role"),
        mx.heat(r.synergy,mF,r.name+" with your team's picks so far"),
        tr,
        mx.heat(r.value,mR,"What not knowing the enemy laner adds to "+r.name+"'s score"),
        w,cov]};
    });
    var main=el("div","mx-main");
    mx.table(main,{cols:cols,rows:rows,gap:14,cls:"blind"});
    main.appendChild(keyRow);
    box.appendChild(main);
  }

  function draw(){
    if(!last)return;
    var d=last.d,s=last.s,blind=last.view==="blind",box=$("recs");
    clear(box);
    box.className="mx"+(blind?" mx-full":" mx-split");
    renderLocked(blind?null:d.locked);
    keyEl.textContent=blind?KEY_BLIND.replace("{role}",s.role||"laner"):KEY_RECS;
    if(blind)renderBlind(box,d.blind||[]);else renderRecs(box,d);
  }

  /* view: "blind" (1b) or "best" (1c), the tab client.js shows (the draft bar is views/draftbar.js) */
  function render(d,s,view){last={d:d,s:s,view:view};draw()}
  L.section("picks",{render:render});
})(window.LPH);
