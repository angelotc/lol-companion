/* Client screen section: champ select teams, enemy lane picker, and the ranked picks as a table:
   picks left to right, and under each one every term of its score (strength, each matchup and
   synergy, your own record). */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,icon=L.icon,fmt=L.fmt,cls=L.cls,clear=L.clear;
  var pickId=null,lastEnemies=[];
  var KIND={vs:"vs",with:"w/"};
  var KIND_ORDER={vs:0,with:1,you:2,mastery:3};
  var KEY_RECS="Δ = win-rate change in that matchup or duo · small = points it adds to the score (shrunk by games, weighted by lane)";
  var KEY_BLIND="Enemy {role} not picked yet: ranked by strength, team fit, and what each loses if they counter it";
  function pts(n){n=Number(n)||0;return n>=1e6?(n/1e6).toFixed(1)+"M":n>=1e3?Math.round(n/1e3)+"k":String(n)}
  function personal(c){return c.kind==="you"||c.kind==="mastery"}
  function key(c){return personal(c)?c.kind:c.kind+":"+c.name}
  function laneIdx(l){var i=L.LANE_ORDER.indexOf(l);return i<0?9:i}

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
      }else ch.appendChild(el("div","lane",lane(c.lane)));
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
  function renderLocked(l){
    var b=$("locked");clear(b);b.classList.toggle("hidden",!l);
    if(!l)return;
    b.appendChild(icon(l.name,l.icon));
    b.appendChild(el("span",null,"Locked in: "+l.name));
    b.appendChild(el("span","sub2","picks frozen"));
  }

  /* one cell: a term of rec `r`'s score */
  function termCell(r,c){
    var lw=" → "+fmt(c.weighted)+" to the score";
    if(c.kind==="mastery"){
      var m=el("span","mval");m.appendChild(el("b",null,pts(c.delta)));m.appendChild(el("i",null,fmt(c.weighted)));
      m.title=pts(c.delta)+" mastery points on "+r.name+lw;
      return m;
    }
    var t=c.kind==="you"?"Your games on "+r.name+": "+fmt(c.delta)+" win rate vs usual in "+c.games+" games":
      r.name+" "+(c.kind==="with"?"with ":"vs ")+c.name+" ("+lane(c.lane)+"): "+fmt(c.delta)+" win rate in "+c.games+" games";
    return L.mxVal(c.delta,c.weighted,fmt(c.weighted),t+lw+(c.tentative?" (still hovering: counts less)":""));
  }
  function rowHead(c){
    if(c.kind==="you")return L.mxLabel("","Your games");
    if(c.kind==="mastery")return L.mxLabel("","Mastery");
    var h=L.mxLabel(KIND[c.kind]||c.kind,c.name,c.icon,c.lane);
    if(c.tentative)h.appendChild(el("span","ln","hover"));
    return h;
  }
  function cols(list,locked){
    return list.map(function(r,i){return{
      head:L.mxHead(i+1,r.name,r.icon,fmt(r.score,2),cls(r.score)),
      cls:(i===0?"top":"")+(locked&&locked.name===r.name?" mine":"")}});
  }
  function renderRecs(recs,locked){
    recs=recs||[];
    var seen={},order=[];
    recs.forEach(function(r){(r.contributions||[]).forEach(function(c){
      var k=key(c);if(!seen[k]){seen[k]=c;order.push(k)}
    })});
    order.sort(function(a,b){var x=seen[a],y=seen[b];return(KIND_ORDER[x.kind]-KIND_ORDER[y.kind])||(laneIdx(x.lane)-laneIdx(y.lane))});
    var rows=[{head:L.mxLabel("","Strength"),cls:"base",cells:recs.map(function(r){
      return L.mxVal(r.base,r.base,null,r.name+": "+fmt(r.base,2)+" win rate vs an average pick in this role, before this draft")})}];
    order.forEach(function(k){
      rows.push({head:rowHead(seen[k]),cls:seen[k].tentative?"tent":"",cells:recs.map(function(r){
        var c=(r.contributions||[]).filter(function(x){return key(x)===k})[0];
        return c?termCell(r,c):null;
      })});
    });
    L.matrix($("recs"),cols(recs,locked),rows);
  }

  function renderBlind(list){
    list=list||[];
    var row=function(name,f,c){return{head:L.mxLabel("",name),cells:list.map(f),cls:c||""}};
    var rows=[
      row("Strength",function(r){var v=r.score-r.value-r.synergy;return L.mxVal(v,v,null,r.name+": overall strength in this role")},"base"),
      row("Team fit",function(r){return L.mxVal(r.synergy,r.synergy,null,r.name+" with your team's picks so far")}),
      row("Avg matchup",function(r){return L.mxVal(r.mean,r.mean,null,r.name+" vs a random enemy laner (pick-rate weighted)")}),
      row("If countered",function(r){return L.mxVal(r.tail,r.tail,null,r.name+" vs its worst matchups (15% of enemy picks)")}),
      row("Blind risk",function(r){return L.mxVal(r.value,r.value,null,"What not knowing the enemy laner adds to "+r.name+"'s score")}),
      row("Worst vs",function(r){
        var w=el("span","mics");
        (r.counters||[]).forEach(function(x){var ic=icon(x.name,x.icon);ic.title=x.name+" "+fmt(x.delta)+" ("+x.games+" games)";w.appendChild(ic)});
        return w;
      }),
      row("Matchup data",function(r){
        var t=el("span","mtxt"+(r.coverage<50?" thin":""),r.coverage+"%");
        t.title=r.coverage<50?"Few games at this tier: the counter risk is partly a typical value":"Share of enemy picks with matchup data";
        return t;
      })
    ];
    L.matrix($("recs"),cols(list,null),rows);
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

  /* view: "blind" (1b) or "best" (1c), the tab client.js shows; the team strip renders either way */
  function render(d,s,view){
    $("note").textContent=d.note||"";$("note").classList.toggle("hidden",!d.note);
    renderOdds(d.odds);
    chipRow($("allies"),d.allies,"ally");
    renderEnemies(d.enemies);
    renderDamage(d.enemy_damage);
    clear($("bans"));$("bansBox").classList.toggle("hidden",!(d.bans&&d.bans.length));
    (d.bans||[]).forEach(function(b){var i=icon(b.name,b.icon);i.title=b.name;$("bans").appendChild(i)});
    renderLocked(d.locked);
    var blind=view==="blind";
    $("recKey").textContent=blind?KEY_BLIND.replace("{role}",s.role||"laner"):KEY_RECS;
    if(blind)renderBlind(d.blind);else renderRecs(d.recs,d.locked);
  }
  L.section("picks",{render:render});
})(window.LPH);
