#include "api.h"

APIFactoryWrap::APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                               bool debug, const std::string &logFileName, std::string usage,
                               const std::string &username, const std::string &password,
                               const cfapi::UserEventHandler &userHandler,
                               const cfapi::SessionEventHandler &sessionHandler)
{
    ptr = cfapi::APIFactory::getInstance();
    ptr->initialize(appName, appVersion, debug, logFileName, usage);
    primaryUser = &ptr->createUserInfo(username, password, const_cast<cfapi::UserEventHandler &>(userHandler));
    session = &ptr->createSession(*primaryUser, const_cast<cfapi::SessionEventHandler &>(sessionHandler));
    printf("api init done.\n");
}

APIFactoryWrap::~APIFactoryWrap()
{
    cfapi::APIFactory::getInstance()->destroySession(*session);
    cfapi::APIFactory::getInstance()->destroyUserInfo(*primaryUser);
    cfapi::APIFactory::getInstance()->uninitialize();
};

void APIFactoryWrap::setSessionConfig(cfapi::SessionConfig::Parameters param, long value)
{
    cfapi::SessionConfig &sessionConfig = (*session).getSessionConfig();
    sessionConfig.set(param, value);
};

void APIFactoryWrap::setConnectionConfig(cfapi::HostConfig::Parameters param, long value)
{
    cfapi::ConnectionConfig &connectionConfig = (*session).getConnectionConfig();
    connectionConfig.set(param, value);
};

void APIFactoryWrap::setHostConfig(std::string &host_info, bool backup, bool compression)
{
    cfapi::ConnectionConfig &connectionConfig = (*session).getConnectionConfig();
    cfapi::HostConfig &hostConfig = connectionConfig.getHostConfig(host_info);
    hostConfig.set(cfapi::HostConfig::BACKUP_BOOL, backup);
    hostConfig.set(cfapi::HostConfig::COMPRESSION_BOOL, compression);
};

void APIFactoryWrap::registerMessageEventHandler(const cfapi::MessageEventHandler &messageHandler)
{
    (*session).registerMessageEventHandler(&const_cast<cfapi::MessageEventHandler &>(messageHandler));
};

std::int64_t APIFactoryWrap::sendRequest(const std::string &src_id, const std::string &symbol, cfapi::Commands command)
{
    cfapi::Request &req = (*session).createRequest();
    req.clearRequest();
    req.add(cfapi::ENUM_SRC_ID, src_id);
    req.add(cfapi::SYMBOL_TICKER, symbol);
    req.setCommand(command);
    // std::int64_t ret = ;
    return (*session).send(req);
};
bool APIFactoryWrap::startSession()
{
    std::string failReason;
    bool ret = (*session).start(failReason);
    if (!ret)
    {
        printf("session could not be established: %s\n", failReason.c_str());
    }
    return ret;
};

// const cfapi::Session& APIFactoryWrap::getSession() {
//     return *session;
// }

// cfapi::Session *APIFactoryWrap::getSession()
// {
//     return session;
// }

void *GetEventReader(const cfapi::MessageEvent &event)
{
    return &event.getReader();
};

// const cfapi::DateTime GetDatetime(cfapi::MessageReader &reader)
// {
//     return reader.getValueAsDateTime();
// };

void *GetDate(const cfapi::MessageReader &reader)
{
    return &((const_cast<cfapi::MessageReader &>(reader)).getValueAsDateTime().date());
};

void *GetTime(const cfapi::MessageReader &reader)
{
    return &((const_cast<cfapi::MessageReader &>(reader)).getValueAsDateTime().time());
};