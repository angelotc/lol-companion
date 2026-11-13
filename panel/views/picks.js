/* Client screen section: champ select teams, enemy lane picker, and ranked recommendations. */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,icon=L.icon,fmt=L.fmt,cls=L.cls,clear=L.clear,short=L.short;
  var openSet={},pickId=null,lastEnemies=[];
  var KIND={vs:"vs",with:"w/",you:"you",mastery:"mastery"};
  function pts(n){n=Number(n)||0;return n>=1e6?(n/1e6).toFixed(1)+"M":n>=1e3?Math.round(n/1e3)+"k":String(n)}
  function personal(c){return c.kind==="you"||c.kind==="mastery"}

  function chipRow(box,list,kind){
    clear(box);
    (list||[]).forEach(function(c){
      var ch=el("div","chip "+kind+(c.tentative?" tent":"")+(c.is_me?" me":""));
      ch.title=c.name+(c.lane?" ("+c.lane+")":"");
      ch.appendChild(icon(c.name,c.icon));
      if(kind==="enemy"){
        ch.className+=(c.flex?" flex":"")+(pickId===c.id?" sel":"");
        ch.title=c.name+" ("+(c.manual?"set to ":"guessed ")+(c.lane||"?")+")"+(c.flex?" - flex pick":"")+": click to set lane";
        ch.appendChild(el("div","lane"+(c.manual?" manual":""),lane(c.lane)));
        ch.addEventListener("click",function(){pickId=pickId===c.id?null:c.id;renderEnemies(lastEnemies)});
      }
      box.appendChild(ch);
    });
  }
  function renderDamage(d){
    var box=$("enemyDmg");clear(box);
    box.classList.toggle("hidden",!d);
    if(!d)return;
    box.appendChild(el("div",null,"Enemy damage: "+d.ap+"% AP · "+d.ad+"% AD"));
    var bar=el("div","bar"),ap=el("div","ap"),ad=el("div","ad");
    ap.style.width=d.ap+"%";ad.style.width=d.ad+"%";
    bar.appendChild(ap);bar.appendChild(ad);box.appendChild(bar);
  }
  function renderEnemies(list){
    lastEnemies=list||[];
    if(pickId!=null&&!lastEnemies.some(function(c){return c.id===pickId}))pickId=null;
    chipRow($("enemies"),lastEnemies,"enemy");
    $("enemyHint").textContent=lastEnemies.length?"· click to set lane":"";
    var box=$("lanePick");clear(box);
    var c=lastEnemies.filter(function(e){return e.id===pickId})[0];
    box.classList.toggle("hidden",!c);
    if(!c)return;
    var t=el("div","lp-t");t.appendChild(el("b",null,c.name));
    t.appendChild(document.createTextNode(c.manual?" · lane set by you":" · lane guessed from play rate"));
    box.appendChild(t);
    var row=el("div","lp-b");
    function choose(l){
      pickId=null;renderEnemies(lastEnemies);
      L.api("set_enemy_lane",c.id,l);
    }
    L.LANE_ORDER.forEach(function(l){
      var b=el("button",c.lane===l?"on":null,lane(l));
      var r=c.rates&&c.rates[l];b.appendChild(el("span",null,r!=null?r+"%":""));
      b.title=c.name+" plays "+l+" in "+(r!=null?r+"%":"?")+" of games";
      b.addEventListener("click",function(){choose(l)});
      row.appendChild(b);
    });
    var a=el("button",!c.manual?"on":null,"auto");a.appendChild(el("span",null,"guess"));
    a.title="Go back to the guessed lane";
    a.addEventListener("click",function(){if(c.manual)choose("");else{pickId=null;renderEnemies(lastEnemies)}});
    row.appendChild(a);
    box.appendChild(row);
  }
  function contrib(c){
    var d=el("div","c");
    d.appendChild(el("span","d "+cls(c.weighted),fmt(c.weighted)));
    if(c.kind==="you"){d.appendChild(el("span",null,"you · "+c.games+" games"));return d}
    if(c.kind==="mastery"){d.appendChild(el("span",null,"mastery · "+pts(c.delta)));return d}
    d.appendChild(el("span",null,KIND[c.kind]||c.kind));
    d.appendChild(icon(c.name,c.icon));
    d.appendChild(el("span","n",short(c.name)));
    return d;
  }
  function detail(r){
    var box=el("div","detail"),t=document.createElement("table");
    var h=document.createElement("tr");
    ["Kind","Champ","Lane","Δ","Games","Wtd"].forEach(function(x){h.appendChild(el("th",null,x))});
    t.appendChild(h);
    (r.contributions||[]).forEach(function(c){
      var tr=document.createElement("tr");if(c.tentative)tr.className="tent";
      tr.appendChild(el("td",null,KIND[c.kind]||c.kind));
      var td=el("td","ch");
      if(personal(c))td.appendChild(el("span","n",c.kind==="you"?"your games":"points"));
      else{td.appendChild(icon(c.name,c.icon));td.appendChild(el("span","n",c.name))}
      tr.appendChild(td);
      tr.appendChild(el("td",null,lane(c.lane)));
      if(c.kind==="mastery")tr.appendChild(el("td",null,pts(c.delta)));
      else tr.appendChild(el("td",cls(c.delta),fmt(c.delta)));
      tr.appendChild(el("td",null,c.kind==="mastery"?"-":c.games!=null?String(c.games):"-"));
      tr.appendChild(el("td",null,fmt(c.weighted)));
      t.appendChild(tr);
    });
    box.appendChild(t);
    var b=el("div","base");b.appendChild(el("span",null,"Base"));b.appendChild(el("span",null,fmt(r.base,2)));
    box.appendChild(b);
    return box;
  }
  function renderLocked(l){
    var b=$("locked");clear(b);b.classList.toggle("hidden",!l);
    if(!l)return;
    b.appendChild(icon(l.name,l.icon));
    b.appendChild(el("span",null,"Locked in: "+l.name));
    b.appendChild(el("span","sub2","picks frozen"));
  }
  function renderRecs(recs,locked){
    var box=$("recs");clear(box);
    (recs||[]).forEach(function(r,i){
      var d=el("div","rec"+(i===0?" top":"")+(openSet[r.name]?" open":"")+(locked&&locked.name===r.name?" mine":""));
      var m=el("div","main");
      m.appendChild(el("div","rank",String(i+1)));
      m.appendChild(icon(r.name,r.icon,"big"));
      var mid=el("div","mid");mid.appendChild(el("div","name",r.name));
      var sub=el("div","sub");
      (r.contributions||[]).slice(0,2).forEach(function(c){sub.appendChild(contrib(c))});
      mid.appendChild(sub);m.appendChild(mid);
      m.appendChild(el("div","score "+cls(r.score),fmt(r.score,2)));
      d.appendChild(m);d.appendChild(detail(r));
      d.addEventListener("click",function(){
        if(openSet[r.name]){delete openSet[r.name];d.classList.remove("open")}
        else{openSet[r.name]=1;d.classList.add("open")}
      });
      box.appendChild(d);
    });
  }

  var mode="blind";
  function renderMode(d,s){
    var box=$("recMode");clear(box);
    var has=!!(d.blind&&d.blind.length)&&!d.locked;
    box.classList.toggle("hidden",!has);
    if(!has)return false;
    box.appendChild(el("span",null,"Enemy "+(s.role||"laner")+" unknown"));
    var pill=el("div","pill");
    [["blind","Blind-safe"],["best","Best now"]].forEach(function(m){
      var b=el("button",mode===m[0]?"on":null,m[1]);
      b.title=m[0]==="blind"?"Ranked by strength, your team, and how each pick holds up if the enemy "+(s.role||"laner")+" counters it":
        "Ranked by strength, the enemies already picked and your team, without counter risk";
      b.addEventListener("click",function(){mode=m[0];render(d,s)});
      pill.appendChild(b);
    });
    box.appendChild(pill);
    return mode==="blind";
  }
  function blindDetail(r){
    var box=el("div","detail"),t=document.createElement("table");
    var h=document.createElement("tr");
    ["Worst vs","Δ","Games"].forEach(function(x){h.appendChild(el("th",null,x))});
    t.appendChild(h);
    (r.counters||[]).forEach(function(c){
      var tr=document.createElement("tr"),td=el("td","ch");
      td.appendChild(icon(c.name,c.icon));td.appendChild(el("span","n",c.name));tr.appendChild(td);
      tr.appendChild(el("td",cls(c.delta),fmt(c.delta)));
      tr.appendChild(el("td",null,String(c.games)));
      t.appendChild(tr);
    });
    box.appendChild(t);
    [["Random opponent",fmt(r.mean,2)],["If countered",fmt(r.tail,2)],["Blind risk",fmt(r.value,2)],
     ["Strength",fmt(r.score-r.value-r.synergy,2)],["Team fit",fmt(r.synergy,2)],["Matchup data",r.coverage+"% of picks"]]
    .forEach(function(x){var b=el("div","base");b.appendChild(el("span",null,x[0]));b.appendChild(el("span",null,x[1]));box.appendChild(b)});
    return box;
  }
  function renderBlind(list){
    var box=$("recs");clear(box);
    (list||[]).forEach(function(r,i){
      var key="b:"+r.name;
      var d=el("div","rec"+(i===0?" top":"")+(openSet[key]?" open":""));
      var m=el("div","main");
      m.appendChild(el("div","rank",String(i+1)));
      m.appendChild(icon(r.name,r.icon,"big"));
      var mid=el("div","mid");mid.appendChild(el("div","name",r.name));
      var sub=el("div","sub"),c=el("div","c");
      var tl=Math.abs(r.tail)<0.05?0:r.tail;
      c.appendChild(el("span","d "+cls(tl),fmt(tl)));
      c.appendChild(el("span",null,(r.counters&&r.counters.length)?"if countered by":"if countered"));
      (r.counters||[]).forEach(function(x){var ic=icon(x.name,x.icon);ic.title=x.name+" "+fmt(x.delta);c.appendChild(ic)});
      sub.appendChild(c);
      if(r.coverage<50){var w=el("div","c thin","only "+r.coverage+"% of matchups known");w.title="Few games at this tier: the counter risk is partly a typical value";sub.appendChild(w)}
      mid.appendChild(sub);m.appendChild(mid);
      var side=el("div","side");
      side.appendChild(el("div","score "+cls(r.score),fmt(r.score,2)));
      var tm=el("div","fit");tm.appendChild(document.createTextNode("team "));
      tm.appendChild(el("span",cls(r.synergy),fmt(r.synergy)));
      tm.title="Fit with your team's picks so far (included in the score)";
      side.appendChild(tm);m.appendChild(side);
      d.appendChild(m);d.appendChild(blindDetail(r));
      d.addEventListener("click",function(){
        if(openSet[key]){delete openSet[key];d.classList.remove("open")}
        else{openSet[key]=1;d.classList.add("open")}
      });
      box.appendChild(d);
    });
  }

  var KINDS={strength:"strong pick",matchup:"",synergy:"synergy"};
  function renderOdds(o){
    var box=$("odds");clear(box);box.classList.toggle("hidden",!o);
    if(!o)return;
    var l=el("div","ol");
    l.appendChild(el("span","us","You "+o.win.toFixed(1)+"%"));
    l.appendChild(el("span","mid","draft estimate · "+o.known+"/10"+(o.pending?" · loading "+o.pending:"")));
    l.appendChild(el("span","them",(100-o.win).toFixed(1)+"% Them"));
    box.appendChild(l);
    var bar=el("div","bar"),fill=el("div");fill.style.width=o.win+"%";bar.appendChild(fill);box.appendChild(bar);
    var why=el("div","why");
    (o.reasons||[]).forEach(function(r){
      var d=el("div");d.appendChild(el("b",cls(r.value),fmt(r.value)));
      (r.icons||[]).forEach(function(u,i){if(i)d.appendChild(el("span",null,r.kind==="matchup"?"vs":"+"));d.appendChild(icon("",u))});
      d.appendChild(el("span",null,(KINDS[r.kind]?KINDS[r.kind]+": ":"")+r.label));
      why.appendChild(d);
    });
    box.appendChild(why);
    box.title="Estimated from each champion's strength, lane matchups and synergy (shrunk by games). "+
      "Uncalibrated until backtesting has enough of your games.";
  }

  function render(d,s){
    $("note").textContent=d.note||"";$("note").classList.toggle("hidden",!d.note);
    renderOdds(d.odds);
    chipRow($("allies"),d.allies,"ally");
    renderEnemies(d.enemies);
    renderDamage(d.enemy_damage);
    clear($("bans"));$("bansBox").classList.toggle("hidden",!(d.bans&&d.bans.length));
    (d.bans||[]).forEach(function(b){var i=icon(b.name,b.icon);i.title=b.name;$("bans").appendChild(i)});
    renderLocked(d.locked);
    if(renderMode(d,s))renderBlind(d.blind);else renderRecs(d.recs,d.locked);
  }
  L.section("picks",{render:render});
})(window.LPH);
