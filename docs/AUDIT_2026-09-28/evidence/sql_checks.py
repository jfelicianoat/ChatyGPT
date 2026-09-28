"""Reproducciones sobre SQLite en memoria y SQL extraído del código vigente.

No abre la base personal, no conecta con servicios ni modifica código de producto.
Las aserciones confirman defectos actuales, no su corrección.
"""
from pathlib import Path
import json
import re
import sqlite3
import unittest

ROOT = Path(__file__).resolve().parents[3]
DB = ROOT / 'apps/desktop/src-tauri/src/db'

def query(file, function, prefix):
    text = (DB / file).read_text(encoding='utf-8').split('pub fn ' + function + '(', 1)[1]
    return next(s for s in re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', text) if s.startswith(prefix))

class Audit(unittest.TestCase):
    def setUp(self):
        self.c = sqlite3.connect(':memory:')
        self.c.execute('PRAGMA foreign_keys=ON')
        for migration in sorted((ROOT / 'apps/desktop/src-tauri/migrations').glob('*.sql')):
            self.c.executescript(migration.read_text(encoding='utf-8'))
        self.c.execute("INSERT INTO conversations(id,title) VALUES ('c','Árbol')")
        self.c.execute("INSERT INTO scheduled_tasks(id,name,schedule_expression,timezone,payload_json,enabled,confirmed_at,next_run_at) VALUES ('s','Prueba','daily','UTC',?,1,datetime('now'),'2020-01-01T10:00:00Z')", (json.dumps({'conversation_id':'c','prompt':'Ejemplo'}),))

    def tearDown(self):
        self.c.close()

    def test_due_query_selects_task_with_active_run(self):
        self.c.execute("INSERT INTO scheduled_runs(id,scheduled_task_id,due_at,claim_key,status) VALUES ('r','s','2019-12-31','manual-key','running')")
        sql = query('programacion_ejecucion.rs','claim_due_scheduled_task','SELECT id, next_run_at')
        self.assertEqual('s', self.c.execute(sql).fetchone()[0])

    def test_recovery_returns_claim_even_when_unlinked_chat_task_exists(self):
        self.c.execute("INSERT INTO scheduled_runs(id,scheduled_task_id,due_at,claim_key,status) VALUES ('r','s','2020-01-01','claim-key','claimed')")
        self.c.execute("INSERT INTO broker_tasks(id,conversation_id,idempotency_key,request_json,remote_status) VALUES ('task','c','already-created','{}','queued')")
        sql = query('programacion_ejecucion.rs','recover_claimed_scheduled_runs','SELECT run.id')
        self.assertEqual('r', self.c.execute(sql).fetchone()[0])

    def test_late_poll_sql_reopens_terminal_task(self):
        self.c.execute("INSERT INTO broker_tasks(id,idempotency_key,request_json,remote_status,local_state,terminal_at) VALUES ('t','key','{}','cancelled','terminal','2026-09-28T10:00:00Z')")
        sql = query('tareas.rs','record_remote_state','UPDATE broker_tasks')
        self.c.execute(sql, ('t','generating','polling',None,None,'{}'))
        self.assertEqual(('generating','polling',None), self.c.execute("SELECT remote_status,local_state,terminal_at FROM broker_tasks WHERE id='t'").fetchone())

    def test_delete_schedule_removes_completed_history(self):
        self.c.execute("INSERT INTO scheduled_runs(id,scheduled_task_id,due_at,claim_key,status) VALUES ('r','s','2020-01-01','claim-key','completed')")
        sql = query('programacion.rs','delete_scheduled_task','DELETE FROM scheduled_tasks')
        self.c.execute(sql, ('s',))
        self.assertEqual(0,self.c.execute('SELECT count(*) FROM scheduled_runs').fetchone()[0])

    def test_chat_search_does_not_fold_spanish_accents_or_case(self):
        sql = query('conversaciones.rs','search_conversations','SELECT c.id')
        self.assertEqual([],self.c.execute(sql,('%árbol%',50)).fetchall())
        self.assertEqual([],self.c.execute(sql,('%arbol%',50)).fetchall())
        self.assertEqual(1,len(self.c.execute(sql,('%Árbol%',50)).fetchall()))

if __name__ == '__main__':
    unittest.main(verbosity=2)
