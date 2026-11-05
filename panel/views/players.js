/* In game (Ctrl+Shift+X): both teams' players with rank, season record and recent form.
   Hover a row for details (the panel never takes focus from the game). */
(function(L){
  var $=L.$,el=L.el,icon=L.icon,clear=L.clear;
  var BAD=/^First time|losses in a row/i;

  function row(p){
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
    var nm=el("span","pl-n",p.name||"?");
    l1.appendChild(nm);
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

  L.register("players",{render:function(s){
    var box=$("plBody");clear(box);
    (s.teams||[]).forEach(function(t){
      var sec=el("div","pl-team "+t.side);
      sec.appendChild(el("div","pl-tl",t.label));
      (t.players||[]).forEach(function(p){sec.appendChild(row(p))});
      box.appendChild(sec);
    });
    var k=el("div","ig-key");k.innerHTML="<b>Ctrl+Shift+X</b> hide · <b>Ctrl+X</b> build";box.appendChild(k);
  }});
})(window.LPH);
