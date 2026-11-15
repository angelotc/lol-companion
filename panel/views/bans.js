/* Client screen section (1a): enemy champions worth banning, as rows ranked by threat: its pick
   rate and lane, threat, own strength (Base), then how each of your likely picks and each
   teammate's hover does against it. Beside it, "Ban this": why the top candidate leads. */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,mx=L.mx,fmt=L.mx.fmt;
  var keyRow=document.querySelector("#secBans .sechrow"),keyEl=keyRow.querySelector(".mxkey");
  var KEY="Each cell is how that pick's win rate moves against the ban candidate, discounted for small samples and weighted by lane. Coral means it beats that pick, which is what makes it worth banning.";
  var LANE_NAME={top:"top",jungle:"jungle",middle:"mid",bottom:"bot",support:"support"};
  function laneName(l){return LANE_NAME[l]||lane(l)}
  function threat(t){return(t.score*10).toFixed(1)}

  function cols(d,threats){
    var mine={},list=[];
    (d.picks||[]).forEach(function(p){if(!mine[p.name]){mine[p.name]=1;list.push({name:p.name,icon:p.icon,group:"pick"})}});
    var hov={};
    (d.allies||[]).forEach(function(a){if(!a.is_me&&!mine[a.name]&&!hov[a.name]){hov[a.name]=1;list.push({name:a.name,icon:a.icon,lane:a.lane,tent:a.tentative,group:"team"})}});
    /* anyone else a threat is measured against (shouldn't happen): with the hovers */
    threats.forEach(function(t){(t.against||[]).forEach(function(c){
      if(!mine[c.name]&&!hov[c.name]){hov[c.name]=1;list.push({name:c.name,icon:c.icon,group:"team"})}
    })});
    return list;
  }
  function vs(t,name){return(t.against||[]).filter(function(a){return a.name===name})[0]}

  /* "Ban this": the top threat, from what the payload carries */
  function side(d,threats,pcols){
    var t=threats[0],box=el("div","mx-side ban");
    var hd=el("div","dt-h");
    hd.appendChild(L.icon(t.name,t.icon,"dt-pt"));
    var nm=el("div","dt-nm");
    nm.appendChild(el("span","dt-k","BAN THIS"));
    nm.appendChild(el("span","dt-n",t.name));
    var nx=threats[1];
    if(nx&&nx.score>0&&t.score>0){
      var ratio=t.score/nx.score;
      nm.appendChild(el("span","dt-s",ratio>=1.15?ratio.toFixed(1)+"× the threat of "+nx.name:"Close to "+nx.name+" ("+threat(nx)+")"));
    }
    hd.appendChild(nm);
    var sc=el("div","dt-sc");sc.appendChild(el("div","v neg",threat(t)));sc.appendChild(el("div","u","threat"));
    hd.appendChild(sc);
    box.appendChild(hd);
    box.appendChild(el("div","dt-th1 cap","Threat = how often you face it × how much it hurts"));

    var facts=el("div","bn-facts");
    function fact(title,sub,right,tip){
      var f=el("div","bn-f");var tx=el("div","tx");tx.appendChild(el("div","t",title));if(sub)tx.appendChild(el("div","s",sub));
      f.appendChild(tx);if(right)f.appendChild(right);if(tip)f.title=tip;facts.appendChild(f);
    }
    /* how often: pick rate, against the most picked on the list */
    var maxPr=Math.max.apply(null,threats.map(function(x){return x.pick_rate}));
    var bar=el("span","bn-bar"),bi=el("i");bi.style.width=(maxPr>0?t.pick_rate/maxPr*100:0).toFixed(0)+"%";bar.appendChild(bi);
    fact("Picked in "+t.pick_rate+"% of games",
      t.pick_rate>=maxPr&&threats.length>1?"Most picked on this list":laneName(t.lane)+" lane",bar,
      "Pick rate in the lanes it plays, summed");
    /* how much it hurts: the pick it beats hardest, then each hover it beats */
    var picks=pcols.filter(function(c){return c.group==="pick"}),team=pcols.filter(function(c){return c.group==="team"});
    var worst=null;
    picks.forEach(function(c){var x=vs(t,c.name);if(x&&x.weighted<0&&(!worst||x.weighted<worst.weighted))worst=x});
    if(worst)fact("Beats "+worst.name,worst.name+" loses "+Math.abs(worst.delta).toFixed(1)+" win-rate points into it ("+worst.games+" games)",
      el("b","bn-v neg",fmt(worst.weighted)));
    team.forEach(function(c){
      var x=vs(t,c.name);
      if(x&&x.weighted<=-0.05)fact("Bad for "+c.name+"’s hover too",c.lane?"Counted with "+laneName(c.lane)+"’s own lane weights":"Counted with that teammate’s lane weights",
        el("b","bn-v neg",fmt(x.weighted)),c.name+" vs "+t.name+": "+fmt(x.delta)+" win rate in "+x.games+" games");
    });
    var strong=t.strength>=0;
    fact(strong?"Strong on its own":"Weak on its own",(strong?"Above":"Below")+" the average win rate for "+laneName(t.lane),
      el("b","bn-v",fmt(t.strength)));
    box.appendChild(facts);

    /* who's next, and how your picks do into it */
    if(nx){
      var beats=[],loses=[];
      picks.forEach(function(c){var x=vs(nx,c.name);if(!x)return;if(x.weighted<=-0.05)beats.push(c.name);else if(x.weighted>=0.05)loses.push(c.name)});
      var s="If "+t.name+" gets banned first, "+nx.name+" is next";
      if(beats.length)s+=": it beats "+join(beats.slice(0,2));
      if(loses.length)s+=(beats.length?", though ":": ")+join(loses.slice(0,2))+" "+(loses.length>1?"do":"does")+" well into it";
      box.appendChild(el("div","bn-next",s+"."));
    }
    return box;
  }
  function join(a){return a.length>1?a.slice(0,-1).join(", ")+" and "+a[a.length-1]:a[0]}

  L.section("bans",{render:function(d){
    var threats=(d&&d.threats)||[],box=$("threats");
    L.clear(box);box.className="mx mx-split";
    keyEl.textContent=KEY;
    $("secBans").title="";
    var pc=cols(d||{},threats);
    var vals=[];threats.forEach(function(t){(t.against||[]).forEach(function(c){vals.push(c.weighted)})});
    var max=mx.maxAbs(vals,1),top=mx.maxAbs(threats.map(function(t){return t.score*10}),0.01);
    var cs=[{w:28,head:""},{w:"minmax(0,1fr)",head:"Ban"},{w:96,head:"Threat",align:"r",
        title:"Ban value: how likely you face it × how much it hurts your picks and your team"},
      {w:56,head:"Base",align:"c",title:"Its own strength: win rate vs an average pick in its lane"}];
    pc.forEach(function(c){
      cs.push({w:56,align:"c",group:c.group,head:mx.headIcon(c.name,c.icon,c.tent,c.name+(c.group==="pick"?", one of your likely picks":", "+(c.lane?laneName(c.lane)+" ":"")+"hover"))});
    });
    var rows=threats.map(function(t,i){
      var cells=[mx.rank(i+1),mx.who(t.name,t.icon,laneName(t.lane).toUpperCase()+" · picked "+t.pick_rate+"%"),
        mx.score(threat(t),t.score*10,top,"neg"),
        mx.plain(fmt(t.strength),t.name+" ("+laneName(t.lane)+"): "+fmt(t.strength,2)+" win rate vs an average pick in its lane")];
      pc.forEach(function(c){
        var x=vs(t,c.name);
        cells.push(x?mx.heat(x.weighted,max,c.name+" vs "+t.name+": "+fmt(x.delta)+" win rate in "+x.games+" games → "+fmt(x.weighted)+" to its ban value"):null);
      });
      return{key:t.name,cells:cells,cls:i===0?"first ban":""};
    });
    var main=el("div","mx-main");
    mx.table(main,{cols:cs,rows:rows,groups:{pick:{label:"Your likely picks",cls:"g-pick"},team:{label:"Team hovers",cls:"g-with"}},cls:"bans"});
    main.appendChild(keyRow);
    box.appendChild(main);
    if(threats.length)box.appendChild(side(d,threats,pc));
  }});
})(window.LPH);
