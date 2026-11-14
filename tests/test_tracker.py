from desktop import tracker
from desktop.tracker import Rect


def test_ingame_panel_grows_with_the_game_resolution():
    p1080 = tracker.place_ingame(Rect(0, 0, 1920, 1080))
    p1440 = tracker.place_ingame(Rect(0, 0, 2560, 1440))
    assert p1080.scale == 1.0 and p1080.width == tracker.INGAME_BASE
    assert round(p1440.scale, 2) == 1.33 and p1440.width == round(tracker.INGAME_BASE * 1440 / 1080)
    assert p1440.x + p1440.width <= 2560 and p1440.y + p1440.height < 1440 * 0.72  # clear of the minimap


def test_ingame_scale_setting_multiplies_the_size():
    big = tracker.place_ingame(Rect(0, 0, 2560, 1440), size=1.25)
    assert round(big.scale, 3) == round(1440 / 1080 * 1.25, 3)
    assert big.width == round(tracker.INGAME_BASE * big.scale)


FRAME = (16, 39)  # borders and title bar


def page(r: Rect) -> tuple[int, int]:
    return r.width - FRAME[0], r.height - FRAME[1]


def test_dragging_a_side_keeps_the_page_16_9_and_the_opposite_side_put():
    right = tracker.fit_aspect(Rect(100, 50, 1116, 600), FRAME, tracker.WMSZ_RIGHT)
    assert page(right) == (1000, 562) and (right.left, right.top) == (100, 50)
    top = tracker.fit_aspect(Rect(100, 50, 1116, 600), FRAME, tracker.WMSZ_TOP)  # height drives
    w, h = page(top)
    assert h == 511 and abs(w / h - 16 / 9) < 0.01 and (top.left, top.bottom) == (100, 600)


def test_dragging_a_corner_follows_the_axis_that_moved_most_and_pins_the_opposite_corner():
    current = Rect(500, 300, 1796, 1059)  # a 1280x720 page
    taller = Rect(500, 100, 1806, 1059)  # top-left dragged up 200, left 10: height wins
    r = tracker.fit_aspect(taller, FRAME, tracker.WMSZ_TOPLEFT, current)
    assert page(r)[1] == 920 and (r.right, r.bottom) == (1806, 1059)
    wider = Rect(500, 300, 2096, 1069)  # bottom-right dragged right 300, down 10: width wins
    r = tracker.fit_aspect(wider, FRAME, tracker.WMSZ_BOTTOMRIGHT, current)
    assert page(r) == (1580, 889) and (r.left, r.top) == (500, 300)


def test_window_never_shrinks_below_the_minimum_page():
    r = tracker.fit_aspect(Rect(0, 0, 200, 150), FRAME, tracker.WMSZ_BOTTOMRIGHT)
    assert page(r) == (tracker.MIN_PAGE_WIDTH, 360)


def test_default_window_fits_a_small_monitor():
    laptop = Rect(0, 0, 1366, 728)  # 768p minus the taskbar
    r = tracker.window_default(laptop, FRAME)
    assert r.bottom <= 728 and r.right == 1366 and abs(page(r)[0] / page(r)[1] - 16 / 9) < 0.01
    big = tracker.window_default(Rect(0, 0, 2560, 1392), FRAME)
    assert page(big) == tracker.PAGE_SIZE
