/* Demo data: open game.html?demo (or ?demo=loading, &scale=1.2). */
(function(){
  /* dev fallback */
  setTimeout(function(){
    if(window.pywebview||location.search.indexOf("demo")<0)return;
    var q=new URLSearchParams(location.search);
    if(q.get("scale"))window.setScale(q.get("scale"));
    window.setMode("ingame");
    var head={role:"support",tier:"Master+",tier_id:"master_plus",days:14,tiers:[]};
    if(q.get("demo")==="loading"){
      window.updateState(Object.assign({view:"game",status:"active",message:"",
        build:null,build_note:"Waiting for game data...",teams:null,players_note:"Loading players..."},head));
      return;
    }
    var I=function(id){return"https://ddragon.leagueoflegends.com/cdn/16.21.1/img/champion/"+id+".png"};
    var IT=function(id,n,g,own){return{id:id,name:n,icon:"https://ddragon.leagueoflegends.com/cdn/16.21.1/img/item/"+id+".png",gold:g,owned:!!own}};
    var V=function(n,id,l,w){return{name:n,icon:I(id),lane:l,weighted:w}};
    var P=function(n,ch,lane,rank,tier,wr,sg,cg,cw,kda,rec,tags,me){return{name:n,tag:"EUW",me:!!me,lane:lane,champ:{name:ch,icon:I(ch)},rank:rank,tier:tier,queue:"solo",rank_full:rank,wr:wr,season_games:sg,champ_games:cg,champ_wr:cw,kda:kda,kills:5,deaths:4,assists:6,games:cg,recent:rec,mastery:42000,tags:tags||[],error:false}};
    window.updateState(Object.assign({view:"game",status:"active",message:"",build_note:"",players_note:"",
      build:{level:9,gold:1350,time:1122,champ:{name:"Leona",icon:I("Leona")},
        keystone:{name:"Aftershock",icon:"https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/VeteranAftershock/VeteranAftershock.png"},
        core:[IT(3876,"Solstice Sleigh",400,1),IT(3111,"Mercury's Treads",1250,1),IT(3190,"Locket of the Iron Solari",2200),IT(3109,"Knight's Vow",2300)],
        next:IT(3190,"Locket of the Iron Solari",2200),
        sections:[
          {title:"Boots",options:[
            {parts:[IT(3111,"Mercury's Treads",1250,1)],wr:51.2,pr:44.0,games:4100,matchup:0.74,has_vs:true,yours:true,vs:[V("Nautilus","Nautilus","support",0.5),V("Caitlyn","Caitlyn","bottom",0.24)]},
            {parts:[IT(3047,"Plated Steelcaps",1200)],wr:51.8,pr:21.0,games:1900,matchup:1.1,has_vs:true,vs:[V("Caitlyn","Caitlyn","bottom",1.1)]},
            {parts:[IT(3158,"Ionian Boots of Lucidity",900)],wr:50.4,pr:15.0,games:1200,matchup:-0.4,has_vs:true,vs:[V("Caitlyn","Caitlyn","bottom",-0.4)]}]},
          {title:"4th item",state:"next",options:[
            {parts:[IT(3109,"Knight's Vow",2300)],wr:52.4,pr:31.0,games:1840,matchup:1.42,has_vs:true,vs:[V("Caitlyn","Caitlyn","bottom",0.9),V("Nautilus","Nautilus","support",0.52)]},
            {parts:[IT(3050,"Zeke's Convergence",2200)],wr:51.1,pr:18.2,games:990,matchup:-0.61,has_vs:true,vs:[V("Nautilus","Nautilus","support",-0.61)]},
            {parts:[IT(3107,"Redemption",2300)],wr:50.6,pr:12.5,games:610,matchup:0.1,has_vs:true,vs:[]}]},
          {title:"5th item",options:[
            {parts:[IT(2065,"Shurelya's Battlesong",2200)],wr:53.0,pr:14.0,games:420,matchup:0.8,has_vs:true,vs:[V("Caitlyn","Caitlyn","bottom",0.8)]},
            {parts:[IT(3083,"Warmog's Armor",3100)],wr:51.9,pr:9.1,games:300,matchup:0,has_vs:false,vs:[]}]}],
        skill_order:"QEWEERQEQQRQQWWRWW",
        opponents:[{name:"Nautilus",icon:I("Nautilus"),lane:"support"},{name:"Caitlyn",icon:I("Caitlyn"),lane:"bottom"}]},
      teams:[
        {side:"ally",label:"Your team",players:[P("Me","Leona","support","G1 67 LP","gold",64,170,9,56,2.7,"WWLWLWWLWW",["Main"],1),P("Foxfire","Ahri","middle","E1 88 LP","emerald",43,202,7,71,5.3,"LLWWWLLWLL",["3 wins in a row"])]},
        {side:"enemy",label:"Enemy team",players:[P("Bugs","Zed","middle","D4 0 LP","diamond",19,109,0,null,null,"LLLLWLLLLL",["First time"])]}]},head));
  },500);
})();
