//! Choose a bounded cursor approach from measured interaction error.
use super::{
    activity::SyncGroup,
    approach,
    timing::{LeadSchedule, SpeedBump, SpeedSchedule},
    Solver, SourcePath, State,
};
use anyhow::Result;

const ACCEPTED: f64 = 0.75;
const SPEEDS: [f64; 16] = [
    1.0, 1.25, 1.6, 2.0, 2.5, 3.2, 4.0, 5.0, 6.4, 8.0, 10.0, 13.0, 16.0, 20.0, 25.0, 32.0,
];

pub struct Context<'a> {
    pub anchor: f64,
    pub anchor_lead: f64,
    pub state: State,
    pub lead: &'a LeadSchedule,
    pub speed: &'a SpeedSchedule,
}

#[derive(Clone)]
pub struct Choice {
    pub lead: f64,
    pub speed: f64,
    pub error: f64,
    pub end_state: State,
    pub knots: Vec<(f64, f64)>,
    pub bump: Option<SpeedBump>,
}
impl Choice {
    pub fn end_lead(&self) -> f64 {
        self.knots.last().map_or(0.0, |p| p.1)
    }
    pub fn commit(&self, lead: &mut LeadSchedule, speed: &mut SpeedSchedule) -> Result<()> {
        for &(time, value) in &self.knots {
            lead.add(time, value)?;
        }
        if let Some(bump) = self.bump {
            speed.add(bump)?;
        }
        Ok(())
    }
}

pub struct Evaluator<'a> {
    path: &'a SourcePath,
    solver: &'a Solver<'a>,
    group: &'a SyncGroup,
    context: Context<'a>,
    window_start: f64,
    committed: Option<Vec<State>>,
}
impl<'a> Evaluator<'a> {
    pub fn new(
        path: &'a SourcePath,
        solver: &'a Solver<'a>,
        group: &'a SyncGroup,
        context: Context<'a>,
    ) -> Self {
        let window_start = context.anchor.max(group.start - 2100.0).floor();
        Self {
            path,
            solver,
            group,
            context,
            window_start,
            committed: None,
        }
    }

    fn committed_at(&mut self, time: f64) -> Result<State> {
        if self.committed.is_none() {
            let c = &self.context;
            let mut lead = c.lead.clone();
            for (time, value) in approach::decay(self.path, c.anchor, c.anchor_lead) {
                lead.add(time, value)?;
            }
            let state = self.solver.advance(
                c.anchor,
                self.window_start,
                c.state,
                &|t| lead.at(t),
                &|t| c.speed.at(t),
                |_, _| {},
            );
            let count = (self.group.start.ceil() - self.window_start + 1.0).max(1.0) as usize;
            let mut states = Vec::with_capacity(count);
            states.push(state);
            for index in 1..count {
                let start = self.window_start + index as f64 - 1.0;
                let next = self.solver.advance(
                    start,
                    start + 1.0,
                    states[index - 1],
                    &|t| lead.at(t),
                    &|t| c.speed.at(t),
                    |_, _| {},
                );
                states.push(next);
            }
            self.committed = Some(states);
        }
        let states = self.committed.as_ref().unwrap();
        Ok(states[((time - self.window_start).max(0.0) as usize).min(states.len() - 1)])
    }

    pub fn score(&mut self, lead_ms: f64, rate: f64, limit: f64) -> Result<Option<Choice>> {
        let c = &self.context;
        let Some(candidate) = approach::candidate(
            self.path,
            c.anchor,
            c.anchor_lead,
            self.group.start,
            self.group.end,
            lead_ms,
        ) else {
            return Ok(None);
        };
        let bump = (rate > 1.0).then_some(SpeedBump {
            ramp_in_start: c.anchor.max(self.group.start - 250.0),
            full_start: self.group.start,
            full_end: self.group.end,
            ramp_out_end: self.group.end + 300.0,
            speed: rate,
        });
        let mut lead = c.lead.clone();
        let mut speed = c.speed.clone();
        for &(t, value) in &candidate.knots {
            lead.add(t, value)?;
        }
        if let Some(b) = bump {
            speed.add(b)?;
        }
        let changes = candidate
            .changes_from
            .min(bump.map_or(f64::INFINITY, |b| b.ramp_in_start))
            .min(self.group.start);
        let mut time = changes.floor().max(self.window_start);
        let mut state = self.committed_at(time)?;
        let mut error: f64 = 0.0;
        for point in &self.group.points {
            state = self.solver.advance(
                time,
                point.time_ms,
                state,
                &|t| lead.at(t),
                &|t| speed.at(t),
                |_, _| {},
            );
            time = point.time_ms;
            let target = self.path.at(time);
            error = error.max(
                (state.position[0] - target[0]).hypot(state.position[1] - target[1])
                    / point.tolerance,
            );
            if error > limit {
                break;
            }
        }
        Ok(Some(Choice {
            lead: lead_ms,
            speed: rate,
            error,
            end_state: state,
            knots: candidate.knots,
            bump,
        }))
    }

    fn refine(
        &mut self,
        mut low: f64,
        mut high: f64,
        mut best: Choice,
        rate: f64,
    ) -> Result<Choice> {
        for _ in 0..6 {
            if high - low <= 0.5 {
                break;
            }
            let middle = (low + high) / 2.0;
            if let Some(choice) = self
                .score(middle, rate, f64::INFINITY)?
                .filter(|c| c.error <= ACCEPTED)
            {
                best = choice;
                high = middle;
            } else {
                low = middle;
            }
        }
        Ok(best)
    }

    fn minimize(
        &mut self,
        mut low: f64,
        mut high: f64,
        mut best: Choice,
        rate: f64,
    ) -> Result<Choice> {
        let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
        for _ in 0..12 {
            if high - low <= 0.25 {
                break;
            }
            let left = high - (high - low) * ratio;
            let right = low + (high - low) * ratio;
            let a = self.score(left, rate, f64::INFINITY)?;
            let b = self.score(right, rate, f64::INFINITY)?;
            let ea = a.as_ref().map_or(f64::INFINITY, |c| c.error);
            let eb = b.as_ref().map_or(f64::INFINITY, |c| c.error);
            if let Some(choice) = a.filter(|c| c.error < best.error) {
                best = choice;
            }
            if let Some(choice) = b.filter(|c| c.error < best.error) {
                best = choice;
            }
            if best.error <= ACCEPTED {
                return Ok(best);
            }
            if ea < eb {
                high = right;
            } else {
                low = left;
            }
        }
        Ok(best)
    }

    fn choose_lead(&mut self, rate: f64) -> Result<Choice> {
        let mut leads = vec![0.0];
        let mut next =
            self.solver.spring.friction / self.solver.spring.tension * 1000.0 / rate / 8.0;
        // Zero damping has zero lag; do not double zero indefinitely.
        while next > 0.0 && next < 600.0 {
            leads.push(next);
            next *= 2.0;
        }
        leads.push(600.0);
        let mut best: Option<(usize, Choice)> = None;
        for (index, &lead) in leads.iter().enumerate() {
            let previous = best.as_ref().map_or(f64::INFINITY, |(_, c)| c.error);
            if let Some(choice) = self.score(lead, rate, previous.max(ACCEPTED))? {
                if choice.error <= ACCEPTED {
                    return if index == 0 {
                        Ok(choice)
                    } else {
                        self.refine(leads[index - 1], lead, choice, rate)
                    };
                }
                if choice.error < previous {
                    best = Some((index, choice));
                }
            }
        }
        if let Some((index, choice)) = best {
            self.minimize(
                leads[index.saturating_sub(1)],
                leads[(index + 1).min(leads.len() - 1)],
                choice,
                rate,
            )
        } else {
            self.score(0.0, rate, f64::INFINITY)?
                .ok_or_else(|| anyhow::anyhow!("No valid cursor approach"))
        }
    }

    pub fn choose(&mut self) -> Result<Choice> {
        let first = self.choose_lead(SPEEDS[0])?;
        if first.error <= ACCEPTED {
            return Ok(first);
        }
        let mut low = 0;
        let mut high = SPEEDS.len() - 1;
        let last = self.choose_lead(SPEEDS[high])?;
        if last.error > ACCEPTED {
            return Ok(if last.error < first.error {
                last
            } else {
                first
            });
        }
        let mut best = last;
        while high - low > 1 {
            let middle = (low + high) / 2;
            let choice = self.choose_lead(SPEEDS[middle])?;
            if choice.error <= ACCEPTED {
                high = middle;
                best = choice;
            } else {
                low = middle;
            }
        }
        Ok(best)
    }
}

#[cfg(test)]
#[path = "optimize_tests.rs"]
mod tests;
