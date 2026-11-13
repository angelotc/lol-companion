/* In game (Ctrl+X): both teams' players on the left, the build planned before the game (with what you
   already own) on the right. Hover for details (the window never takes focus from the game). Only your
   own items and gold are used, never the enemies'. */
(function(L){
  var $=L.$,el=L.el,icon=L.icon,fmt=L.fmt,cls=L.cls,clear=L.clear;
  var SIG=0.3,BAD=/^First time|losses in a row/i;

  function k(n){n=Number(n)||0;return n>=1000?(n/1000).toFixed(1)+"k":String(n)}
  function clock(s){s=Number(s)||0;var m=Math.floor(s/60),r=Math.floor(s%60);return m+":"+(r<10?"0":"")+r}
  function itemIcon(p){
    var ic=icon(p.name,p.icon);ic.title=p.name+(p.gold?" · "+p.gold+"g":"");
    if(p.owned)ic.classList.add("own");
    return ic;
  }
  function note(box,text,err){box.appendChild(el("div","gnote"+(err?" err":""),text||""))}

  function build(box,s){
    var h=el("div","ig-h");
    if(s.champ)h.appendChild(icon(s.champ.name,s.champ.icon,"igc"));
    var t=el("div","ig-t");
    t.appendChild(el("div","ig-n",(s.champ?s.champ.name:"")+" · lvl "+(s.level||"?")));
    t.appendChild(el("div","ig-s",clock(s.time)+" · "+k(s.gold)+" gold"));
    h.appendChild(t);
    if(s.keystone){var ks=icon(s.keystone.name,s.keystone.icon,"igk");ks.title=s.keystone.name;h.appendChild(ks)}
    box.appendChild(h);
    if(s.damage)box.appendChild(L.damage(s.damage));
    if(s.empty){box.appendChild(el("div","bnote",s.empty));return}

    if(s.core&&s.core.length){
      var c=el("div","ig-sec");c.appendChild(el("div","ig-l","Core"));
      var row=el("div","ig-core");
      s.core.forEach(function(p,i){if(i)row.appendChild(el("span","arrow","›"));row.appendChild(itemIcon(p))});
      c.appendChild(row);
      if(s.next){
        var nx=el("div","ig-next"),need=Math.max(0,(s.next.gold||0)-(s.gold||0));
        nx.appendChild(el("span",null,"Next: "+s.next.name));
        nx.appendChild(el("b",need?"":"pos",need?k(need)+" more":"affordable"));
        c.appendChild(nx);
      }
      box.appendChild(c);
    }
    var cols=(s.opponents||[]).slice().sort(function(a,b){return L.LANE_ORDER.indexOf(a.lane)-L.LANE_ORDER.indexOf(b.lane)});
    var grid="repeat("+cols.length+",1.75rem)";
    (s.sections||[]).forEach(function(sec){
      var b=el("div","ig-sec"+(sec.state?" "+sec.state:""));
      var lh=el("div","ig-lh");
      lh.appendChild(el("div","ig-l",sec.title+(sec.state==="next"?" · next":"")));
      var any=sec.options.some(function(o){return (o.vs||[]).length});
      if(cols.length&&any){
        var hc=el("span","ig-vs");hc.style.gridTemplateColumns=grid;
        cols.forEach(function(c){var ci=icon(c.name,c.icon);ci.title="vs "+c.name+" ("+c.lane+")";hc.appendChild(ci)});
        lh.appendChild(hc);lh.appendChild(el("span","ig-mu"));
      }
      b.appendChild(lh);
      sec.options.forEach(function(o){
        var r=el("div","ig-opt"+(o.yours?" yours":""));
        r.title=(o.parts||[]).map(function(p){return p.name}).join(" + ")+" · "+o.wr+"% win · "+o.pr+"% picked"
          +(o.games?" · "+k(o.games)+" games":"");
        (o.parts||[]).forEach(function(p){r.appendChild(itemIcon(p))});
        var wr=el("span","ig-wr");
        wr.appendChild(el("span",null,o.wr+"%"));
        r.appendChild(wr);
        if(o.yours)r.appendChild(el("span","ig-tag","YOURS"));
        var m=Number(o.matchup)||0;
        if(cols.length&&any){  // one fixed column per enemy lane, so each enemy's deltas line up
          var parts=el("span","ig-vs");parts.style.gridTemplateColumns=grid;
          cols.forEach(function(c){
            var v=(o.vs||[]).filter(function(x){return x.lane===c.lane})[0];
            var d=el("b",v?cls(v.weighted):"",v?fmt(v.weighted):"");
            if(v)d.title=v.name+" ("+v.lane+"): "+fmt(v.weighted)+" of this option's edge";
            parts.appendChild(d);
          });
          r.appendChild(parts);
        }
        r.appendChild(el("span","ig-mu "+(o.has_vs&&Math.abs(m)>=SIG?cls(m):""),o.has_vs?(Math.abs(m)>=SIG?fmt(m):"±0"):""));
        b.appendChild(r);
      });
      box.appendChild(b);
    });
    if(s.skill_order){
      var sk=el("div","ig-sec");sk.appendChild(el("div","ig-l","Skills"));
      var ord=el("div","skillord");
      s.skill_order.split("").forEach(function(ch,i){
        ord.appendChild(el("span",(ch==="R"?"R":"")+(i+1===s.level?" now":"")+(i+1<s.level?" done":""),ch));
      });
      sk.appendChild(ord);box.appendChild(sk);
    }
    if(s.opponents&&s.opponents.length){
      var v=el("div","vsrow");v.appendChild(el("span",null,"Δ vs"));
      s.opponents.forEach(function(o){var i=icon(o.name,o.icon);i.title=o.name;v.appendChild(i)});
      v.appendChild(el("span",null,"· from before the game"));
      box.appendChild(v);
    }
  }

  function player(p){
    var r=el("div","pl-row"+(p.me?" me":"")+(p.error?" err":""));
    var tip=[p.name+(p.tag?"#"+p.tag:"")+(p.lane?" ("+p.lane+")":""),
      p.rank_full+(p.queue==="flex"?" (Flex)":"")];
    if(p.kda!=null)tip.push("Avg K/D/A: "+p.kills+" / "+p.deaths+" / "+p.assists+(p.games?" over "+p.games+" games":""));
    if(p.mastery!=null)tip.push("Mastery: "+p.mastery.toLocaleString()+" pts");
    if(p.error)tip.push("Some data could not be loaded");
    r.title=tip.join("\n");
    r.appendChild(icon(p.champ?p.champ.name:"?",p.champ?p.champ.icon:null,"pl-ico"));
    var b=el("div","pl-b");
    var l1=el("div","pl-l1");
    l1.appendChild(el("span","pl-n",p.name||"?"));
    if(p.rank)l1.appendChild(el("span","pl-rk t-"+(p.tier||"none"),p.rank));
    if(p.wr!=null)l1.appendChild(el("span","pl-wr"+(p.wr>=55?" pos":p.wr<=45?" neg":""),p.wr+"%"+(p.season_games?" ("+p.season_games+")":"")));
    else if(p.wins)l1.appendChild(el("span","pl-wr",p.wins+"W"));  // losses unknown below Master
    b.appendChild(l1);
    if(p.champ_season_games!=null){  // season record on this champion (op.gg)
      var ls=el("div","pl-ls");
      ls.appendChild(el("b",null,p.champ_season_games+" games"));
      if(p.champ_season_games)
        ls.appendChild(el("span","pl-wr"+(p.champ_season_wr>=55?" pos":p.champ_season_wr<=45?" neg":"")," "+p.champ_season_wr+"% WR"));
      b.appendChild(ls);
    }
    var l2=el("div","pl-l2");
    var rec=p.champ_games?(p.champ_season_games!=null?"Recent: ":"")+p.champ_games+" games"+(p.champ_wr!=null?" · "+p.champ_wr+"%":"")+(p.kda!=null?" · "+p.kda+" KDA":"")
      :(p.games?"not in last "+p.games+" games":"no recent games");
    l2.appendChild(el("span","pl-rec",rec));
    var dots=el("span","pl-dots");
    String(p.recent||"").split("").forEach(function(c){dots.appendChild(el("i",c==="W"?"w":"l"))});
    l2.appendChild(dots);
    b.appendChild(l2);
    if(p.tags&&p.tags.length){
      var tg=el("div","pl-tags");
      p.tags.forEach(function(t){tg.appendChild(el("span","pl-tag "+(BAD.test(t)?"bad":"good"),t))});
      b.appendChild(tg);
    }
    r.appendChild(b);
    return r;
  }

  function players(box,teams){
    teams.forEach(function(t){
      var sec=el("div","pl-team "+t.side);
      sec.appendChild(el("div","pl-tl",t.label));
      var ps=t.players||[];
      ps.forEach(function(p){sec.appendChild(player(p))});
      if(!ps.length)sec.appendChild(el("div","bnote","No players"));
      box.appendChild(sec);
    });
  }

  /* States arrive every 2 seconds: rebuild a column without losing its scroll position */
  function fill(id,draw){
    var box=$(id),top=box.scrollTop;
    clear(box);draw(box);
    box.scrollTop=top;
  }

  L.register("game",{render:function(s){
    fill("gPlayers",function(box){if(s.teams)players(box,s.teams);else note(box,s.players_note)});
    fill("gBuild",function(box){if(s.build)build(box,s.build);else note(box,s.build_note,s.status==="error")});
  }});
})(window.LPH);
