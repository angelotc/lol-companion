from data.leaderboard import parse

ROW = ('<div class="flex h-[52px] justify-between text-[13px]" q:key="0_{i}">'
       '<div class="flex w-[50px]">{i}</div><div><img alt="challenger"><div class="t">CH</div></div>'
       '<div class="flex w-[150px]"><a href="https://xdx.gg/x">{name}</a></div><div>euw</div>'
       '<div>60.00%</div><div>70</div><div>x 70</div></div>')


def test_parse_rows_and_opgg_link():
    page = "<div>Rank</div>" + ROW.format(i=1, name="Some One#EUW") + ROW.format(i=2, name="A&amp;B#1")
    a, b = parse(page)
    assert (a.rank, a.tier, a.riot_id, a.region, a.wr, a.games) == (1, "CH", "Some One#EUW", "euw", 60.0, 70)
    assert a.profile == "https://xdx.gg/x"
    assert a.opgg == "https://op.gg/lol/summoners/euw/Some%20One-EUW"
    assert b.riot_id == "A&B#1"


def test_parse_skips_unknown_markup():
    assert parse("<html>changed layout</html>") == []
