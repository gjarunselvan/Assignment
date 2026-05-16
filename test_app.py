import unittest

from app import handle_request


class AppTests(unittest.TestCase):
    def test_returns_gists_for_octocat(self):
        status, body = handle_request(
            "/octocat",
            lambda user: [
                {
                    "id": "1",
                    "description": "octocat gist",
                    "html_url": "https://gist.github.com/octocat/1",
                    "files": ["hello.txt"],
                }
            ],
        )

        self.assertEqual(status, 200)
        self.assertEqual(body[0]["id"], "1")

    def test_returns_not_found_for_missing_user(self):
        status, body = handle_request(
            "/missing-user",
            lambda user: (_ for _ in ()).throw(
                ValueError("GitHub user 'missing-user' was not found")
            ),
        )

        self.assertEqual(status, 404)
        self.assertEqual(
            body,
            {"error": "GitHub user 'missing-user' was not found"},
        )


if __name__ == "__main__":
    unittest.main()
