use super::super::World;
impl World {
    pub(in crate::world) fn hash_residents(&self, mix: &mut impl FnMut(u64)) {
        mix(self.residents.rng.state());
        mix(self.residents.population as u64);
        mix(self.residents.registered as u64);
        mix(self
            .residents
            .connectivity_version
            .map_or(u64::MAX, u64::from));
        mix(self.residents.component.len() as u64);
        for &component in &self.residents.component {
            mix(component as u64);
        }
        mix(self.homes.building.len() as u64);
        for h in 0..self.homes.building.len() {
            mix(self.homes.building[h] as u64);
            mix(self.homes.capacity[h] as u64);
            mix(self.homes.household[h].map_or(u64::MAX, u64::from));
        }
        mix(self.households.active.len() as u64);
        for h in 0..self.households.active.len() {
            mix(self.households.active[h] as u64);
            mix(self.households.home[h] as u64);
            mix(self.households.members[h].len() as u64);
            for &c in &self.households.members[h] {
                mix(c as u64);
            }
        }
        let c = &self.citizens;
        mix(c.active.len() as u64);
        for i in 0..c.active.len() {
            mix(c.active[i] as u64);
            mix(c.household[i] as u64);
            mix(c.job[i].map_or(u64::MAX, u64::from));
            mix(c.location[i] as u64);
            mix(c.destination[i].map_or(u64::MAX, u64::from));
            mix(c.activity[i] as u64);
            mix(c.agent[i].map_or(u64::MAX, u64::from));
            mix(c.due[i]);
            mix(c.job_due[i]);
            mix(c.departure[i] as u64);
            mix(c.work_duration[i] as u64);
        }
        for (&jobs, &workers) in self.residents.jobs.iter().zip(&self.residents.workers) {
            mix(jobs as u64);
            mix(workers as u64);
        }
        for heap in [
            &self.residents.activities,
            &self.residents.searches,
            &self.residents.arrivals,
        ] {
            mix(heap.len() as u64);
            for entry in heap.clone().into_sorted_vec() {
                mix(entry.0.0);
                mix(entry.0.1 as u64);
            }
        }
    }
}
