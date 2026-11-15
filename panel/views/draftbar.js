/* The champ select shell around the views: the draft bar (your team | draft win estimate | their
   team, with the enemy lane picker) and the right side of the step row (their damage, bans).
   Renders from the picks payload, which is there for the whole draft. */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,icon=L.icon,fmt=L.fmt,cls=L.cls,clear=L.clear;
  var pickId=null,lastEnemies=[];
  var SHORT=L.LANE_ORDER.map(lane);  // top jg mid bot sup: the five slots, left to right
  function capOf(l){return(lane(l)||"?").toUpperCase()}

  /* five slots: each champion in its lane's slot when free, the rest in the free ones left to right */
  function place(list){
    var out=[null,null,null,null,null],rest=[];
    list.forEach(function(c){var i=SHORT.indexOf(lane(c.lane));if(i>=0&&!out[i])out[i]=c;else rest.push(c)});
    rest.forEach(function(c){var i=out.indexOf(null);if(i>=0)out[i]=c});
    return out;
  }
  function portrait(c){return c?icon(c.name,c.icon,"pt"):el("span","ico pt")}

  function allySlot(c,i){
    if(!c){var e=el("div","slot ally open");e.appendChild(portrait());e.appendChild(el("span","cap",SHORT[i].toUpperCase()));
      e.title="Not picked yet";return e}
    var d=el("div","slot ally"+(c.tentative?" tent":"")+(c.is_me?" me":""));
    var has=!c.is_me||!!c.icon;  // your own slot is there before you hover a champion
    var p=has?portrait(c):el("span","ico pt");
    if(c.is_me&&!has)p.appendChild(el("span","you","YOU"));
    d.appendChild(p);
    d.appendChild(el("span","cap",c.is_me&&has?"YOU":capOf(c.lane)+(c.tentative&&!c.is_me?"?":"")));
    d.title=c.is_me?(has?"You: "+c.name+(c.tentative?" (hovering)":""):"You ("+(c.lane||"?")+")"):
      c.name+(c.lane?" ("+c.lane+")":"")+(c.tentative?" - hovering":"");
    return d;
  }
  function enemySlot(c,i){
    if(!c){var e=el("div","slot enemy open");e.appendChild(portrait());e.appendChild(el("span","cap",SHORT[i].toUpperCase()));
      e.title="Not picked yet";return e}
    var d=el("div","slot enemy"+(c.flex?" flex":"")+(pickId===c.id?" sel":""));
    d.appendChild(portrait(c));
    d.appendChild(el("span","cap"+(c.manual?" manual":""),capOf(c.lane)+(c.flex&&!c.manual?"?":"")));
    d.title=c.name+" ("+(c.manual?"set to ":"guessed ")+(c.lane||"?")+")"+(c.flex?" - flex pick":"")+": click to set lane";
    d.addEventListener("click",function(){pickId=pickId===c.id?null:c.id;renderEnemies(lastEnemies)});
    return d;
  }

  function renderAllies(list,role){
    list=(list||[]).slice();
    if(!list.some(function(c){return c.is_me}))list.push({name:"",icon:"",lane:role||"",is_me:true});
    var box=$("allies");clear(box);
    place(list).forEach(function(c,i){box.appendChild(allySlot(c,i))});
  }
  function renderEnemies(list){
    lastEnemies=list||[];
    if(pickId!=null&&!lastEnemies.some(function(c){return c.id===pickId}))pickId=null;
    var row=$("enemies");clear(row);
    place(lastEnemies).forEach(function(c,i){row.appendChild(enemySlot(c,i))});
    var box=$("lanePick");clear(box);
    var c=lastEnemies.filter(function(e){return e.id===pickId})[0];
    box.classList.toggle("hidden",!c);
    if(!c)return;
    var t=el("div","lp-t");t.appendChild(el("b",null,c.name));
    t.appendChild(document.createTextNode(c.manual?" · lane set by you":" · lane guessed from play rate"));
    box.appendChild(t);
    var btns=el("div","lp-b");
    function choose(l){
      pickId=null;renderEnemies(lastEnemies);
      L.api("set_enemy_lane",c.id,l);
    }
    L.LANE_ORDER.forEach(function(l){
      var b=el("button",c.lane===l?"on":null,lane(l));
      var r=c.rates&&c.rates[l];b.appendChild(el("span",null,r!=null?r+"%":""));
      b.title=c.name+" plays "+l+" in "+(r!=null?r+"%":"?")+" of games";
      b.addEventListener("click",function(){choose(l)});
      btns.appendChild(b);
    });
    var a=el("button",!c.manual?"on":null,"auto");a.appendChild(el("span",null,"guess"));
    a.title="Go back to the guessed lane";
    a.addEventListener("click",function(){if(c.manual)choose("");else{pickId=null;renderEnemies(lastEnemies)}});
    btns.appendChild(a);
    box.appendChild(btns);
  }

  /* a reason's text: "Jinx is strong", "Lee Sin vs Viego", "Jinx + Ahri" */
  function reason(r){
    if(r.kind!=="strength")return r.label;
    /* value is in your favour, so an enemy's strength counts negative: find whose champion it is */
    var theirs=lastEnemies.some(function(e){return e.icon&&e.icon===(r.icons||[])[0]});
    var strong=theirs?Number(r.value)<0:Number(r.value)>=0;
    return String(r.label||"").replace(/\s*\([^)]*\)$/,"")+(strong?" is strong":" is weak");
  }
  function renderOdds(o,note){
    var box=$("odds");clear(box);
    var win=o?Number(o.win):50,pct=function(v){return v.toFixed(1)+"%"};
    var l=el("div","ol");
    var us=el("span","us",pct(win)+" ");us.appendChild(el("small",null,"you"));
    var them=el("span","them");them.appendChild(el("small",null,"them"));them.appendChild(document.createTextNode(" "+pct(100-win)));
    l.appendChild(us);
    l.appendChild(el("span","mid","draft estimate · "+(o?o.known:0)+" of 10 picked"+(o&&o.pending?" · loading "+o.pending:"")));
    l.appendChild(them);
    box.appendChild(l);
    var bar=el("div","bar"),fill=el("div");fill.style.width=win+"%";bar.appendChild(fill);box.appendChild(bar);
    var reasons=(o&&o.reasons)||[];
    if(reasons.length||note){
      var why=el("div","why");
      reasons.forEach(function(r){
        var c=el("span","chip");c.appendChild(el("b",cls(r.value),fmt(r.value)));
        c.appendChild(document.createTextNode(reason(r)));
        c.title=(r.kind==="strength"?"Champion strength: ":r.kind==="matchup"?"Matchup: ":"Synergy: ")+r.label+" "+fmt(r.value);
        why.appendChild(c);
      });
      if(note){var n=el("span","chip warn",note);why.appendChild(n)}
      box.appendChild(why);
    }else box.appendChild(el("div","hint","The estimate moves once champions are locked in"));
    box.title="Estimated from each champion's strength, lane matchups and synergy (shrunk by games). "+
      "Uncalibrated until backtesting has enough of your games.";
  }

  function renderDamage(d){
    var box=$("enemyDmg");clear(box);box.classList.toggle("hidden",!d);
    if(!d)return;
    box.appendChild(el("span","st-lbl","Their damage"));
    var bar=el("div","bar"),ad=el("div","ad");ad.style.width=d.ad+"%";bar.appendChild(ad);bar.appendChild(el("div","ap"));
    box.appendChild(bar);
    var t=el("span");t.appendChild(el("b","ad",d.ad+"% AD"));t.appendChild(document.createTextNode(" · "));t.appendChild(el("b","ap",d.ap+"% AP"));
    box.appendChild(t);
    box.title="The enemy team's damage split, from each enemy's average damage to champions in its lane";
  }
  /* bans so far; during the ban phase the open slots show as dashed boxes */
  function renderBans(list,banning){
    var box=$("bans");clear(box);list=list||[];
    $("bansBox").classList.toggle("hidden",!list.length&&!banning);
    list.forEach(function(b){var i=icon(b.name,b.icon);i.title=b.name+" (banned)";box.appendChild(i)});
    for(var n=list.length;banning&&n<5;n++)box.appendChild(el("span","open"));
  }

  L.section("draftbar",{render:function(d,s){
    renderAllies(d.allies,s.role);
    renderEnemies(d.enemies);
    renderOdds(d.odds,d.note);
    renderDamage(d.enemy_damage);
    renderBans(d.bans,!!s.bans);
  }});
})(window.LPH);
