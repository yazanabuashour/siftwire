# SiftWire terminology

SiftWire fetches configured sources, selects items, and prepares briefs locally.
Agents own scheduling and delivery.

## Terms

### Brief collection and delivery

**Source outcome**:
The result of one fetch: collected items, sports updates, selection and
suppression evidence, health warnings, and an optional next source marker.
A source outcome describes permitted state changes without writing them.

**Prepared delivery plan**:
The selection and rendered bodies saved for one brief run. A saved plan cannot
change. Confirmation records the plan's exact message and items, not a new
delivery assembled from caller-supplied content.

### Sports coverage

**Fixture**:
A scheduled future match or event in a followed competition. A calendar fact,
not news and not a live score.
_Avoid_: game, upcoming event, match (ambiguous with played matches)

**Fixture line**:
The sports-section entry that presents a fixture: competition, teams, and
kickoff time.
_Avoid_: schedule entry, upcoming card

**Followed competition**:
A configured schedule source tracking one team's or league's upcoming fixtures.
_Avoid_: watched team, sports subscription

**Result line**:
The sports-section entry that presents a completed fixture and its final score
when the provider supplies one.
_Avoid_: live score, post-game summary

**Result recap**:
A news item reporting the narrative of a completed fixture. It remains a normal
brief item and is separate from the structured result line.
_Avoid_: result line, live score

**Live score**:
Real-time match state. Explicitly out of scope for SiftWire; external services
own it.
_Avoid_: score feed, in-game update
