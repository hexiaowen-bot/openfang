// OpenFang Orchestrator Page — Task orchestration and agent pool management
'use strict';

function orchestratorPage() {
  return {
    // === State ===
    tasks: [],
    agents: [],
    selectedTask: null,
    loading: false,
    submitting: false,
    
    // Submit form
    taskDescription: '',
    forceComplexity: '',
    lastAnalysis: null,
    lastTaskId: null,
    
    // Auto refresh
    refreshInterval: null,
    
    // Stats
    stats: { total: 0, running: 0, completed: 0, failed: 0, agents: 0, permanent: 0, cached: 0, dynamic: 0 },
    
    async init() {
      await this.refresh();
      var self = this;
      this.refreshInterval = setInterval(function() { self.refresh(); }, 5000);
    },
    
    destroy() {
      if (this.refreshInterval) {
        clearInterval(this.refreshInterval);
        this.refreshInterval = null;
      }
    },
    
    async refresh() {
      await Promise.all([this.loadTasks(), this.loadAgents()]);
      this.updateStats();
    },
    
    async loadTasks() {
      try {
        var res = await OpenFangAPI.get('/api/orchestrator/tasks');
        this.tasks = res || [];
      } catch(e) {
        console.error('Failed to load tasks:', e);
        this.tasks = [];
      }
    },
    
    async loadAgents() {
      try {
        var res = await OpenFangAPI.get('/api/orchestrator/agents');
        this.agents = res || [];
      } catch(e) {
        console.error('Failed to load agents:', e);
        this.agents = [];
      }
    },
    
    updateStats() {
      this.stats = {
        total: this.tasks.length,
        running: this.tasks.filter(function(t) { return t.state === 'Running' || t.state === 'Analyzing' || t.state === 'Preparing' || t.state === 'Monitoring'; }).length,
        completed: this.tasks.filter(function(t) { return t.state === 'Completed'; }).length,
        failed: this.tasks.filter(function(t) { return t.state === 'Failed'; }).length,
        agents: this.agents.length,
        permanent: this.agents.filter(function(a) { return a.retention === 'Permanent'; }).length,
        cached: this.agents.filter(function(a) { return a.retention === 'Cached'; }).length,
        dynamic: this.agents.filter(function(a) { return a.retention === 'Dynamic'; }).length,
      };
    },
    
    async submitTask() {
      if (!this.taskDescription || !this.taskDescription.trim()) return;
      this.submitting = true;
      try {
        var body = {
          description: this.taskDescription,
          force_complexity: this.forceComplexity || null
        };
        var res = await OpenFangAPI.post('/api/orchestrator/tasks', body);
        this.lastTaskId = res.task_id;
        this.lastAnalysis = res.analysis;
        this.taskDescription = '';
        OpenFangToast.success('Task submitted');
        await this.loadTasks();
      } catch(e) {
        OpenFangToast.error('Submit failed: ' + (e.message || 'unknown error'));
      } finally {
        this.submitting = false;
      }
    },
    
    async executeTask(taskId) {
      try {
        await OpenFangAPI.post('/api/orchestrator/tasks/' + taskId + '/execute');
        OpenFangToast.success('Execution started');
        await this.loadTasks();
      } catch(e) {
        OpenFangToast.error('Execute failed: ' + (e.message || 'unknown error'));
      }
    },
    
    async cancelTask(taskId) {
      try {
        await OpenFangAPI.delete('/api/orchestrator/tasks/' + taskId);
        OpenFangToast.success('Task cancelled');
        await this.loadTasks();
      } catch(e) {
        OpenFangToast.error('Cancel failed: ' + (e.message || 'unknown error'));
      }
    },
    
    async viewTask(taskId) {
      try {
        var res = await OpenFangAPI.get('/api/orchestrator/tasks/' + taskId);
        this.selectedTask = res;
      } catch(e) {
        OpenFangToast.error('Failed to load task details');
      }
    },
    
    closeTaskDetail() {
      this.selectedTask = null;
    },
    
    async evaluateRetention(agentId) {
      try {
        var res = await OpenFangAPI.get('/api/orchestrator/agents/' + agentId + '/retention');
        OpenFangToast.success('Score: ' + res.score + ' → ' + res.decision);
      } catch(e) {
        OpenFangToast.error('Evaluation failed');
      }
    },
    
    async cleanupAgents() {
      try {
        var res = await OpenFangAPI.post('/api/orchestrator/agents/cleanup', { max_idle_secs: 3600 });
        var count = res && res.cleaned ? res.cleaned.length : 0;
        OpenFangToast.success('Cleaned ' + count + ' idle agents');
        await this.loadAgents();
      } catch(e) {
        OpenFangToast.error('Cleanup failed');
      }
    },
    
    // === Helper methods ===
    stateColor(state) {
      var colors = {
        'Pending': 'rgba(150,150,150,0.3)',
        'Analyzing': 'rgba(0,200,255,0.3)',
        'Preparing': 'rgba(0,200,255,0.3)',
        'Running': 'rgba(0,150,255,0.3)',
        'Monitoring': 'rgba(0,150,255,0.3)',
        'Completed': 'rgba(0,255,100,0.3)',
        'Failed': 'rgba(255,50,50,0.3)',
        'Cancelled': 'rgba(255,200,0,0.3)'
      };
      return colors[state] || 'rgba(150,150,150,0.3)';
    },
    
    stateTextColor(state) {
      var colors = {
        'Pending': 'var(--text-muted)',
        'Analyzing': 'var(--info)',
        'Preparing': 'var(--info)',
        'Running': 'var(--accent)',
        'Monitoring': 'var(--accent)',
        'Completed': 'var(--success)',
        'Failed': 'var(--error)',
        'Cancelled': 'var(--warning)'
      };
      return colors[state] || 'var(--text-muted)';
    },
    
    retentionColor(retention) {
      var colors = {
        'Permanent': 'rgba(255,200,0,0.3)',
        'Cached': 'rgba(0,150,255,0.3)',
        'Dynamic': 'rgba(150,150,150,0.3)'
      };
      return colors[retention] || 'rgba(150,150,150,0.3)';
    },
    
    retentionTextColor(retention) {
      var colors = {
        'Permanent': 'var(--warning)',
        'Cached': 'var(--info)',
        'Dynamic': 'var(--text-muted)'
      };
      return colors[retention] || 'var(--text-muted)';
    },
    
    complexityLabel(c) {
      var labels = { 'Simple': 'Simple', 'Medium': 'Medium', 'Complex': 'Complex' };
      return labels[c] || c || '-';
    },
    
    formatTime(ts) {
      if (!ts) return '-';
      try {
        return new Date(ts).toLocaleString();
      } catch(e) {
        return String(ts);
      }
    },
    
    successRate(agent) {
      if (!agent.use_count || agent.use_count === 0) return '0%';
      return Math.round((agent.success_count / agent.use_count) * 100) + '%';
    },
    
    truncate(str, maxLen) {
      if (!str) return '';
      if (str.length <= maxLen) return str;
      return str.substring(0, maxLen) + '...';
    }
  };
}
