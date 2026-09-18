"""Search Kernel's transport-independent Python domain models.

The former portable native transport has been retired. A gRPC transport will
be added separately; this package intentionally contains no executable client.
"""

from .generated.models import *  # noqa: F401,F403
from .generated.models import __all__
