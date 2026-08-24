# SiftWire

SiftWire is a local-first brief runtime: it fetches configured sources, selects
items, and delivers short briefs on a fixed schedule.

## Language

### Sports coverage

**Fixture**:
A scheduled future match or event in a followed competition. A calendar fact,
not news and not a live score.
_Avoid_: game, upcoming event, match (ambiguous with played matches)

**Fixture line**:
The brief item that presents a fixture: competition, teams, and kickoff time.
_Avoid_: schedule entry, upcoming card

**Followed competition**:
A configured schedule source tracking one team's or league's upcoming fixtures.
_Avoid_: watched team, sports subscription

**Result recap**:
A news item reporting the outcome of a completed fixture.
_Avoid_: match report, post-game summary

**Live score**:
Real-time match state. Explicitly out of scope for SiftWire; external services
own it.
_Avoid_: score feed, in-game update
