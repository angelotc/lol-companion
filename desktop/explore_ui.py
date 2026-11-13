"""Blind-pick explorer: a section of the client panel, shown between games (not during champ
select). One chart over one role: every champion's strength vs how much it loses if countered."""
from __future__ import annotations

import logging
from typing import Callable

from draft.scoring import LANES
from draft.views import TIERS

log = logging.getLogger(__name__)


class ExploreApi:
    """Backs the panel's explorer section (through the panel's Api)."""

    def __init__(self, report: Callable[[str, str], dict], tier: Callable[[], str],
                 role: Callable[[], str | None] = lambda: None):
        self._report, self._tier, self._role = report, tier, role

    def start(self) -> dict:
        """What to show first: your most played role (else support) at the panel's tier."""
        role = self._role()
        return {"role": role if role in LANES else "support", "tier": self._tier(),
                "roles": list(LANES), "tiers": [{"id": k, "label": v} for k, v in TIERS.items()]}

    def get(self, role: str, tier: str) -> dict:
        if role not in LANES or tier not in TIERS:
            return {"error": "unknown role or tier"}
        try:
            return self._report(role, tier) | {"tier_label": TIERS[tier]}
        except Exception as e:  # network or data trouble: say so in the panel
            log.exception("blind report")
            return {"error": f"Couldn't load stats ({type(e).__name__}). Try again in a moment."}
