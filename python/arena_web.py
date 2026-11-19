"""Serve the offline duel table on loopback."""

import argparse
from collections import OrderedDict
from hashlib import sha256
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
import json
from pathlib import Path
import secrets
from threading import RLock
from urllib.parse import urlsplit

from arena.native import ACTIONS, ARENA_ABI, FEATURES, ROOT, roster
from arena.policy import DEFAULT_CHECKPOINT, Policy
from arena_table import Duel
from arena_play import SUBCLASS_DESCRIPTIONS, subclass_name


class ArenaServer(ThreadingHTTPServer):
    def __init__(self, port, checkpoint):
        data = checkpoint.read_bytes()
        self.policy = Policy.load(BytesIO(data)).require_arena(
            ARENA_ABI, FEATURES, ACTIONS
        )
        if self.policy.metadata.get("stage") == "migrated_warm_start":
            raise ValueError("Для игры нужны обученные веса этой версии арены.")
        self.model = f"{checkpoint.name} · {sha256(data).hexdigest()[:10]}"
        self.games = OrderedDict()
        self.lock = RLock()
        super().__init__(("127.0.0.1", port), Handler)


ASSETS = {
    "": ("index.html", "text/html"),
    "app.js": ("app.js", "text/javascript"),
    "board.js": ("board.js", "text/javascript"),
    "style.css": ("style.css", "text/css"),
}


class Handler(BaseHTTPRequestHandler):
    def reply(self, value, status=200, mime="application/json"):
        raw = (
            value
            if isinstance(value, bytes)
            else json.dumps(value, ensure_ascii=False).encode()
        )
        self.send_response(status)
        self.send_header("Content-Type", mime + "; charset=utf-8")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        path = urlsplit(self.path).path.removeprefix("/")
        if path == "api/roster":
            return self.reply([dict(build, title=subclass_name(build), detail=SUBCLASS_DESCRIPTIONS[build["name"]]) for build in roster()])
        if path not in ASSETS:
            return self.reply({}, 404)
        filename, mime = ASSETS[path]
        self.reply((ROOT / "web" / filename).read_bytes(), mime=mime)

    def do_POST(self):
        host = self.headers.get("Host", "")
        allowed = {f"{h}:{self.server.server_port}" for h in ("127.0.0.1", "localhost")}
        origin = self.headers.get("Origin")
        if host not in allowed or (origin and origin != "http://" + host):
            return self.reply({"error": "Недопустимый источник."}, 403)
        if self.headers.get_content_type() != "application/json":
            return self.reply({}, 415)
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if not 0 < size <= 8192:
                raise ValueError("Неверный размер запроса.")
            body = json.loads(self.rfile.read(size))
            if not isinstance(body, dict):
                raise ValueError("Неверный формат запроса.")
            with self.server.lock:
                data, code = self.dispatch(body)
        except (TypeError, ValueError, KeyError, OverflowError) as error:
            data, code = {"error": str(error)}, HTTPStatus.BAD_REQUEST
        self.reply(data, code)

    def dispatch(self, body):
        games = self.server.games
        if self.path == "/api/new":
            duel = Duel(self.server.policy, **body)
            token = secrets.token_urlsafe(24)
            games[token] = duel
            if len(games) > 32:
                games.popitem(last=False)[1].batch.close()
        else:
            token = body.get("game")
            if token not in games:
                return {"error": "Сессия завершена. Начните новую дуэль."}, 404
            duel = games[token]
            games.move_to_end(token)
            if self.path != "/api/state":
                if body.get("rev") != duel.rev:
                    return {
                        "error": "Позиция уже изменилась.",
                        "view": duel.view(),
                    }, 409
                if self.path == "/api/play":
                    duel.play(body.get("action"), body.get("column"))
                elif self.path == "/api/walk":
                    duel.walk(body.get("destination"))
                elif self.path == "/api/bot":
                    duel.bot()
                else:
                    return {}, 404
        return dict(game=token, model=self.server.model, view=duel.view()), 200


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--checkpoint", type=Path, default=DEFAULT_CHECKPOINT)
    args = parser.parse_args()
    with ArenaServer(args.port, args.checkpoint) as server:
        print(f"Арена открыта на http://127.0.0.1:{server.server_port}/", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
