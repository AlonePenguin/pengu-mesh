use std::{cmp::Ordering, collections::HashMap, time::Instant};

/// Configuration for a task queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskQueueConfig {
    /// Maximum number of tasks the queue can hold.
    pub max_capacity: usize,
    /// Maximum number of tasks a single agent may have enqueued.
    pub max_per_agent: usize,
}

/// A task waiting in the queue.
#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub id: String,
    pub agent_id: String,
    /// Higher values indicate higher priority.
    pub priority: u8,
    pub enqueued_at: Instant,
}

/// Errors returned when enqueuing fails.
#[derive(Debug, PartialEq, Eq)]
pub enum QueueError {
    /// The queue has reached its global capacity limit.
    AtCapacity,
    /// The agent has reached its per-agent slot limit.
    AgentLimitReached,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::AtCapacity => write!(f, "queue is at capacity"),
            QueueError::AgentLimitReached => write!(f, "agent has reached its slot limit"),
        }
    }
}

impl std::error::Error for QueueError {}

/// An in-memory task queue with bounded admission and per-agent dequeue fairness.
///
/// Higher-priority tasks are always selected first. Among tasks with the same
/// priority, the queue prefers agents that have not been served recently so
/// one agent cannot monopolize a priority band by enqueuing a deeper backlog.
/// Ties within the same agent are FIFO by `enqueued_at`.
#[derive(Debug)]
pub struct TaskQueue {
    config: TaskQueueConfig,
    tasks: Vec<QueuedTask>,
    agent_counts: HashMap<String, usize>,
    last_served: HashMap<String, u64>,
    next_service_order: u64,
}

impl TaskQueue {
    pub fn new(config: TaskQueueConfig) -> Self {
        Self {
            config,
            tasks: Vec::new(),
            agent_counts: HashMap::new(),
            last_served: HashMap::new(),
            next_service_order: 0,
        }
    }

    /// Enqueue a task. Returns an error if the global capacity or per-agent
    /// limit would be exceeded.
    pub fn enqueue(&mut self, task: QueuedTask) -> Result<(), QueueError> {
        if self.tasks.len() >= self.config.max_capacity {
            return Err(QueueError::AtCapacity);
        }
        let agent_count = self.agent_counts.get(&task.agent_id).copied().unwrap_or(0);
        if agent_count >= self.config.max_per_agent {
            return Err(QueueError::AgentLimitReached);
        }
        *self.agent_counts.entry(task.agent_id.clone()).or_insert(0) += 1;
        self.tasks.push(task);
        Ok(())
    }

    /// Remove and return the next task.
    ///
    /// Priority is the primary ordering. Tasks with the same priority are
    /// served fairly across agents, and then FIFO within each agent.
    pub fn dequeue(&mut self) -> Option<QueuedTask> {
        if self.tasks.is_empty() {
            return None;
        }

        let mut best = 0;
        for index in 1..self.tasks.len() {
            if self.candidate_beats(&self.tasks[index], &self.tasks[best]) {
                best = index;
            }
        }

        let task = self.tasks.swap_remove(best);
        self.record_service(&task.agent_id);
        self.decrement_agent(&task.agent_id);
        Some(task)
    }

    /// Cancel a task by id. Returns `true` if the task was found and removed.
    pub fn cancel(&mut self, task_id: &str) -> bool {
        if let Some(pos) = self.tasks.iter().position(|task| task.id == task_id) {
            let task = self.tasks.swap_remove(pos);
            self.decrement_agent(&task.agent_id);
            true
        } else {
            false
        }
    }

    /// Number of tasks currently in the queue.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns `true` if the queue contains no tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Number of tasks enqueued for a specific agent.
    pub fn agent_count(&self, agent_id: &str) -> usize {
        self.agent_counts.get(agent_id).copied().unwrap_or(0)
    }

    fn candidate_beats(&self, candidate: &QueuedTask, current_best: &QueuedTask) -> bool {
        if candidate.priority != current_best.priority {
            return candidate.priority > current_best.priority;
        }

        match self.compare_agent_fairness(&candidate.agent_id, &current_best.agent_id) {
            Ordering::Less => return true,
            Ordering::Greater => return false,
            Ordering::Equal => {}
        }

        if candidate.enqueued_at != current_best.enqueued_at {
            return candidate.enqueued_at < current_best.enqueued_at;
        }

        if candidate.agent_id != current_best.agent_id {
            return candidate.agent_id < current_best.agent_id;
        }

        candidate.id < current_best.id
    }

    fn compare_agent_fairness(&self, candidate_agent: &str, current_best_agent: &str) -> Ordering {
        match (
            self.last_served.get(candidate_agent),
            self.last_served.get(current_best_agent),
        ) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(candidate), Some(current_best)) => candidate.cmp(current_best),
        }
    }

    fn record_service(&mut self, agent_id: &str) {
        self.next_service_order = self.next_service_order.saturating_add(1);
        self.last_served
            .insert(agent_id.to_string(), self.next_service_order);
    }

    fn decrement_agent(&mut self, agent_id: &str) {
        if let Some(count) = self.agent_counts.get_mut(agent_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.agent_counts.remove(agent_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_task(id: &str, agent: &str, priority: u8) -> QueuedTask {
        make_task_at(id, agent, priority, Instant::now())
    }

    fn make_task_at(id: &str, agent: &str, priority: u8, enqueued_at: Instant) -> QueuedTask {
        QueuedTask {
            id: id.to_string(),
            agent_id: agent.to_string(),
            priority,
            enqueued_at,
        }
    }

    #[test]
    fn enqueue_and_dequeue_single() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        queue.enqueue(make_task("t1", "a1", 5)).unwrap();
        assert_eq!(queue.len(), 1);

        let task = queue.dequeue().unwrap();
        assert_eq!(task.id, "t1");
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn dequeue_returns_highest_priority() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        queue.enqueue(make_task("low", "a1", 1)).unwrap();
        queue.enqueue(make_task("high", "a1", 10)).unwrap();
        queue.enqueue(make_task("mid", "a1", 5)).unwrap();

        assert_eq!(queue.dequeue().unwrap().id, "high");
        assert_eq!(queue.dequeue().unwrap().id, "mid");
        assert_eq!(queue.dequeue().unwrap().id, "low");
    }

    #[test]
    fn dequeue_fairly_rotates_agents_with_same_priority() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        let base = Instant::now();

        queue
            .enqueue(make_task_at("a1", "agent_a", 5, base))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "a2",
                "agent_a",
                5,
                base + Duration::from_millis(1),
            ))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "b1",
                "agent_b",
                5,
                base + Duration::from_millis(2),
            ))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "b2",
                "agent_b",
                5,
                base + Duration::from_millis(3),
            ))
            .unwrap();

        assert_eq!(queue.dequeue().unwrap().id, "a1");
        assert_eq!(queue.dequeue().unwrap().id, "b1");
        assert_eq!(queue.dequeue().unwrap().id, "a2");
        assert_eq!(queue.dequeue().unwrap().id, "b2");
    }

    #[test]
    fn dequeue_uses_fifo_within_same_agent_and_priority() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        let base = Instant::now();

        queue
            .enqueue(make_task_at("a1", "agent_a", 5, base))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "a2",
                "agent_a",
                5,
                base + Duration::from_millis(1),
            ))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "b1",
                "agent_b",
                5,
                base + Duration::from_millis(2),
            ))
            .unwrap();

        assert_eq!(queue.dequeue().unwrap().id, "a1");
        assert_eq!(queue.dequeue().unwrap().id, "b1");
        assert_eq!(queue.dequeue().unwrap().id, "a2");
    }

    #[test]
    fn higher_priority_beats_fairness_rotation() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        let base = Instant::now();

        queue
            .enqueue(make_task_at("a1", "agent_a", 5, base))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "b1",
                "agent_b",
                5,
                base + Duration::from_millis(1),
            ))
            .unwrap();
        assert_eq!(queue.dequeue().unwrap().id, "a1");

        queue
            .enqueue(make_task_at(
                "a2",
                "agent_a",
                10,
                base + Duration::from_millis(2),
            ))
            .unwrap();
        assert_eq!(queue.dequeue().unwrap().id, "a2");
        assert_eq!(queue.dequeue().unwrap().id, "b1");
    }

    #[test]
    fn fairness_persists_when_an_agent_reenters_the_queue() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        let base = Instant::now();

        queue
            .enqueue(make_task_at("a1", "agent_a", 5, base))
            .unwrap();
        queue
            .enqueue(make_task_at(
                "b1",
                "agent_b",
                5,
                base + Duration::from_millis(1),
            ))
            .unwrap();
        assert_eq!(queue.dequeue().unwrap().id, "a1");

        queue
            .enqueue(make_task_at(
                "a2",
                "agent_a",
                5,
                base + Duration::from_millis(2),
            ))
            .unwrap();

        assert_eq!(queue.dequeue().unwrap().id, "b1");
        assert_eq!(queue.dequeue().unwrap().id, "a2");
    }

    #[test]
    fn rejects_at_capacity() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 2,
            max_per_agent: 5,
        });
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        queue.enqueue(make_task("t2", "a2", 1)).unwrap();

        let err = queue.enqueue(make_task("t3", "a3", 1)).unwrap_err();
        assert_eq!(err, QueueError::AtCapacity);
    }

    #[test]
    fn rejects_agent_limit() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 2,
        });
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        queue.enqueue(make_task("t2", "a1", 2)).unwrap();

        let err = queue.enqueue(make_task("t3", "a1", 3)).unwrap_err();
        assert_eq!(err, QueueError::AgentLimitReached);

        queue.enqueue(make_task("t4", "a2", 1)).unwrap();
    }

    #[test]
    fn cancel_existing_task() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        queue.enqueue(make_task("t2", "a1", 2)).unwrap();

        assert!(queue.cancel("t1"));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.agent_count("a1"), 1);
    }

    #[test]
    fn cancel_nonexistent_returns_false() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        assert!(!queue.cancel("nope"));
    }

    #[test]
    fn agent_count_tracks_correctly() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        assert_eq!(queue.agent_count("a1"), 0);
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        queue.enqueue(make_task("t2", "a1", 2)).unwrap();
        queue.enqueue(make_task("t3", "a2", 1)).unwrap();

        assert_eq!(queue.agent_count("a1"), 2);
        assert_eq!(queue.agent_count("a2"), 1);

        queue.dequeue();
        assert_eq!(queue.agent_count("a1") + queue.agent_count("a2"), 2);
    }

    #[test]
    fn dequeue_empty_returns_none() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 5,
        });
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn cancel_frees_agent_slot() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 1,
        });
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        assert_eq!(
            queue.enqueue(make_task("t2", "a1", 2)).unwrap_err(),
            QueueError::AgentLimitReached
        );

        queue.cancel("t1");
        queue.enqueue(make_task("t3", "a1", 3)).unwrap();
        assert_eq!(queue.agent_count("a1"), 1);
    }

    #[test]
    fn dequeue_frees_agent_slot() {
        let mut queue = TaskQueue::new(TaskQueueConfig {
            max_capacity: 10,
            max_per_agent: 1,
        });
        queue.enqueue(make_task("t1", "a1", 1)).unwrap();
        queue.dequeue();
        queue.enqueue(make_task("t2", "a1", 2)).unwrap();

        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", QueueError::AtCapacity),
            "queue is at capacity"
        );
        assert_eq!(
            format!("{}", QueueError::AgentLimitReached),
            "agent has reached its slot limit"
        );
    }
}
