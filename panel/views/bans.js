/* Client screen section: enemy champions worth banning, as a table: ban candidates left to right,
   and under each how your likely picks and your team's hovers do against it. */
(function(L){
  var $=L.$,el=L.el,lane=L.lane,fmt=L.fmt;

  L.section("bans",{render:function(d){
    var threats=d.threats||[];
    var mine={};(d.picks||[]).forEach(function(p){mine[p.name]=1});
    var hovers={};(d.allies||[]).forEach(function(a){if(!a.is_me)hovers[a.name]=a});
    var seen={},order=[];
    threats.forEach(function(t){(t.against||[]).forEach(function(c){if(!seen[c.name]){seen[c.name]=c;order.push(c.name)}})});
    order.sort(function(a,b){return(mine[a]?0:1)-(mine[b]?0:1)});  // your picks first, then teammates
    var cols=threats.map(function(t,i){return{
      head:L.mxHead(i+1,t.name,t.icon,(t.score*10).toFixed(1),"",lane(t.lane)+" · picked "+t.pick_rate+"%"),
      cls:i===0?"top":""}});
    var rows=[{head:L.mxLabel("","Own strength"),cls:"base",cells:threats.map(function(t){
      return L.mxVal(t.strength,t.strength,null,t.name+" ("+lane(t.lane)+"): win rate vs an average pick in its lane")})}];
    order.forEach(function(name){
      var c=seen[name],h=hovers[name];
      rows.push({head:L.mxLabel(mine[name]?"pick":"team",name,c.icon,h&&h.lane),cells:threats.map(function(t){
        var x=(t.against||[]).filter(function(a){return a.name===name})[0];
        return x?L.mxVal(x.delta,x.weighted,fmt(x.weighted),
          name+" vs "+t.name+": "+fmt(x.delta)+" win rate in "+x.games+" games → "+fmt(x.weighted)+" to its ban value"):null;
      })});
    });
    L.matrix($("threats"),cols,rows);
    $("secBans").title="Ban value: how likely you face it x how much it hurts your picks and your team";
  }});
})(window.LPH);
